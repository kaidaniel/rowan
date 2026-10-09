//! Reports: per-access combination across targets, schema table, text / JSON (port of
//! `rowan/report.py`).

use indexmap::IndexMap;
use serde_json::{json, Value as J};

use crate::domain::{source_is_data, source_label, KeyVal, Ns, ObjId, Row, Tier, Value};
use crate::intern::{KeyId, STAR};
use crate::state::State;
use crate::intern::{self, key_repr};
use crate::interp::{Analyzer, Verdict};
use crate::state::Kind;

/// One verdict per (access site, key, call chain): per-target verdicts are combined (§13).
pub fn dedupe(verdicts: &[Verdict]) -> Vec<Verdict> {
    let mut groups: IndexMap<(String, u32, u32, String, Vec<u32>, Ns, String), Vec<&Verdict>> = IndexMap::new();
    for v in verdicts {
        groups.entry((v.file.clone(), v.line, v.col, key_repr(v.key), v.chain.clone(), v.ns, v.text.clone())).or_default().push(v);
    }
    let mut out: Vec<Verdict> = Vec::new();
    for (_, vs) in groups {
        let mut by_obj: IndexMap<u32, &Verdict> = IndexMap::new();
        for v in &vs {
            match by_obj.get(&v.oid) {
                Some(cur) if v.tier.rank() <= cur.tier.rank() => {}
                _ => {
                    by_obj.insert(v.oid, v);
                }
            }
        }
        let labels: Vec<&str> = by_obj.values().map(|v| v.obj.as_str()).collect();
        let mut uniq = labels.clone();
        uniq.sort();
        uniq.dedup();
        let names: IndexMap<u32, String> = if uniq.len() != labels.len() {
            by_obj.iter().map(|(o, v)| (*o, format!("{}#{}", v.obj, o))).collect()
        } else {
            by_obj.iter().map(|(o, v)| (*o, v.obj.clone())).collect()
        };
        let mut tiers: Vec<Tier> = by_obj.values().map(|v| v.tier).collect();
        tiers.sort();
        tiers.dedup();
        if tiers.contains(&Tier::Unknown) {
            continue;
        }
        let first = vs[0];
        if tiers.len() == 1 {
            let v = by_obj.values().next().unwrap();
            let mut objs: Vec<String> = names.values().cloned().collect();
            objs.sort();
            out.push(Verdict { obj: objs.join(", "), ..(*v).clone() });
            continue;
        }
        let mut sorted: Vec<(&u32, &&Verdict)> = by_obj.iter().collect();
        sorted.sort_by_key(|(o, _)| **o);
        let breakdown: Vec<String> = sorted.iter().map(|(o, v)| format!("{} for {}", v.tier.symbol(), names[*o])).collect();
        let tier = if tiers.iter().all(|t| matches!(t, Tier::Proven | Tier::Likely)) {
            Tier::Likely
        } else if tiers.iter().all(|t| matches!(t, Tier::Input | Tier::Proven | Tier::Likely)) {
            Tier::Input
        } else {
            Tier::Possibly
        };
        let mut provs: Vec<String> = by_obj.values().flat_map(|v| v.providers.clone()).collect();
        provs.sort();
        provs.dedup();
        let mut objs: Vec<String> = names.values().cloned().collect();
        objs.sort();
        out.push(Verdict {
            line: first.line,
            col: first.col,
            key: first.key,
            tier,
            obj: objs.join(", "),
            chain: first.chain.clone(),
            note: format!("depends on which object: {}", breakdown.join("; ")),
            providers: provs,
            ns: first.ns,
            text: first.text.clone(),
            oid: first.oid,
            file: first.file.clone(),
        });
    }
    // the same finding reached through many call chains: keep the shortest few chains
    let mut by_finding: IndexMap<(String, u32, u32, String, Ns, String, Tier, String, String), Vec<Verdict>> = IndexMap::new();
    for v in out {
        by_finding.entry((v.file.clone(), v.line, v.col, key_repr(v.key), v.ns, v.text.clone(), v.tier, v.obj.clone(), v.note.clone())).or_default().push(v);
    }
    let mut out: Vec<Verdict> = Vec::new();
    for (_, mut vs) in by_finding {
        vs.sort_by(|a, b| (a.chain.len(), &a.chain).cmp(&(b.chain.len(), &b.chain)));
        vs.truncate(MAX_CHAINS_PER_FINDING);
        out.extend(vs);
    }
    out.sort_by(|a, b| (&a.file, a.line, a.col, key_repr(a.key), &a.chain).cmp(&(&b.file, b.line, b.col, key_repr(b.key), &b.chain)));
    out
}

/// How many distinct call chains one finding is reported under (the shortest ones).
pub const MAX_CHAINS_PER_FINDING: usize = 8;

pub struct SchemaEntry {
    pub always: bool,
    pub sites: Vec<String>,
    pub alternatives: Vec<String>,
    pub caveats: Vec<String>,
    pub alt_keys: Vec<String>,
}

/// Source label -> {key -> entry}
pub fn schema_rows(an: &Analyzer) -> IndexMap<String, IndexMap<String, SchemaEntry>> {
    let mut out: IndexMap<String, IndexMap<String, SchemaEntry>> = IndexMap::new();
    let Some(st) = &an.final_state else { return out };
    let mut items = st.reqs.items();
    items.sort_by_key(|(s, _)| source_label(*s));
    for (src, d) in items {
        if source_is_data(src) {
            continue;
        }
        let label = source_label(src);
        let entry = out.entry(label).or_default();
        let mut keys: Vec<_> = d.iter().collect();
        keys.sort_by_key(|(k, _)| key_repr(**k));
        for (k, info) in keys {
            let mut sites: Vec<String> = info.sites.iter().map(|s| s.to_string()).collect();
            sites.sort();
            let mut alts: Vec<String> = info.alternatives.iter().map(source_label).collect();
            alts.sort();
            let mut cav: Vec<String> = info.caveats.iter().map(|s| s.to_string()).collect();
            cav.sort();
            let mut ak: Vec<String> = info.alt_keys.iter().map(key_repr).collect();
            ak.sort();
            entry.insert(key_repr(*k), SchemaEntry { always: info.always, sites, alternatives: alts, caveats: cav, alt_keys: ak });
        }
    }
    out
}

fn group_alt_keys(entry: &IndexMap<String, SchemaEntry>) -> Vec<(String, &SchemaEntry)> {
    let mut seen: Vec<String> = Vec::new();
    let mut items = Vec::new();
    let mut keys: Vec<&String> = entry.keys().collect();
    keys.sort();
    for k in keys {
        if seen.contains(k) {
            continue;
        }
        let mut group = vec![k.clone()];
        for a in &entry[k].alt_keys {
            if a != k && entry.contains_key(a) {
                group.push(a.clone());
            }
        }
        for g in &group {
            seen.push(g.clone());
        }
        let label = if group.len() > 1 { group.join(" | ") } else { k.clone() };
        items.push((label, &entry[k]));
    }
    items
}

pub fn format_text(an: &Analyzer, show_rows: bool, show_summaries: bool) -> String {
    let mut lines: Vec<String> = Vec::new();
    let all = dedupe(&an.verdicts);
    // an attribute read from an input is a requirement (listed with the schemas), not a
    // finding: reading attributes of objects from outside is what programs do
    let vs: Vec<&Verdict> = all.iter().filter(|v| !matches!(v.tier, Tier::Proven | Tier::Map) && !(v.tier == Tier::Input && v.ns == Ns::Attr)).collect();
    let oks = all.iter().filter(|v| v.tier == Tier::Proven).count();
    lines.push("== diagnostics ==".into());
    if vs.is_empty() {
        lines.push("  (none)".into());
    }
    for v in vs {
        let chain = if v.chain.is_empty() { String::new() } else { format!("  via {}", v.chain.iter().map(|c| format!("L{}", c)).collect::<Vec<_>>().join(">")) };
        let prov = if v.providers.is_empty() { String::new() } else { format!("  ← {}", v.providers.join(", ")) };
        let note = if v.note.is_empty() { String::new() } else { format!("  ({})", v.note) };
        let wher = if v.file.is_empty() { String::new() } else { format!("{}:", v.file) };
        lines.push(format!("  {}L{:<4} {:<5} {:<40} {}[{}] {}{}{}  [{}]{}", wher, v.line, v.tier.symbol(), v.text, v.ns, key_repr(v.key), v.tier.text(), prov, note, v.obj, chain));
    }
    lines.push(format!("  ({} accesses proven ok)", oks));
    let sch = schema_rows(an);
    lines.push(String::new());
    lines.push("== inferred schemas (requirements on external inputs) ==".into());
    if sch.is_empty() {
        lines.push("  (none)".into());
    }
    let mut labels: Vec<&String> = sch.keys().collect();
    labels.sort();
    for label in labels {
        let entry = &sch[label];
        let grouped = group_alt_keys(entry);
        let always: Vec<String> = grouped.iter().filter(|(_, e)| e.always).map(|(k, _)| k.clone()).collect();
        let sometimes: Vec<String> = grouped.iter().filter(|(_, e)| !e.always).map(|(k, _)| k.clone()).collect();
        lines.push(format!("  {}:", label));
        if !always.is_empty() {
            lines.push(format!("      always   : {}", always.join(", ")));
        }
        if !sometimes.is_empty() {
            lines.push(format!("      sometimes: {}", sometimes.join(", ")));
        }
        for (k, e) in &grouped {
            let mut extra = Vec::new();
            if !e.alternatives.is_empty() {
                extra.push(format!("or provided by {}", e.alternatives.join(", ")));
            }
            if !e.caveats.is_empty() {
                extra.push(format!("unless added by {}", e.caveats.join(", ")));
            }
            if !extra.is_empty() {
                lines.push(format!("      {}: {}", k, extra.join("; ")));
            }
        }
    }
    if show_summaries && !an.summaries.is_empty() {
        lines.push(String::new());
        lines.push("== function results (per call context) ==".into());
        let mut entries: Vec<(&(String, String), &(crate::domain::Value, Vec<(u32, crate::domain::Row)>))> = an.summaries.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for ((qual, ctx), (_, rows)) in entries {
            for (oid, row) in rows {
                lines.push(format!("  {} [{}] -> {}: {}", qual, ctx, an.obj(*oid).label, row));
            }
        }
    }
    if show_rows {
        if let Some(st) = &an.final_state {
            lines.push(String::new());
            lines.push("== module-level values at exit ==".into());
            let m = an.main_module;
            let mut names: Vec<(String, crate::domain::Value)> = st.obj_slots(m, Ns::Attr).into_iter().map(|(k, v)| (intern::key_py_str(k), v)).collect();
            names.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, v) in names {
                // definitions (functions, classes, imported modules) are not values worth listing
                if !v.pts.is_empty() && v.pts.iter().all(|o| matches!(an.kind(o), Kind::Function | Kind::Class | Kind::Module | Kind::External)) {
                    continue;
                }
                let labels: Vec<String> = v.pts.iter().filter(|o| an.kind(*o) != Kind::Unknown).map(|o| an.obj(o).label.clone()).collect();
                let rendered = render_value(an, st, &v, 0, &mut Vec::new());
                if labels.is_empty() {
                    lines.push(format!("  {} = {}", name, rendered));
                } else {
                    lines.push(format!("  {} = {}  [{}]", name, rendered, labels.join(" | ")));
                }
            }
        }
    }
    if !an.notes.is_empty() {
        lines.push(String::new());
        lines.push("== notes ==".into());
        for n in &an.notes {
            lines.push(format!("  {}", n));
        }
    }
    lines.join("\n")
}

pub fn verdict_json(an: &Analyzer, v: &Verdict) -> J {
    json!({
        "kind": "verdict",
        "site": format!("{}:{}:{}", if v.file.is_empty() { an.filename.clone() } else { v.file.clone() }, v.line, v.col),
        "file": v.file,
        "line": v.line,
        "col": v.col,
        "key": key_repr(v.key),
        "key_str": intern::key_py_str(v.key),
        "ns": v.ns.as_str(),
        "tier": v.tier.name(),
        "object": v.obj,
        "chain": v.chain,
        "note": v.note,
        "providers": v.providers,
        "text": v.text,
    })
}

pub fn to_json(an: &Analyzer) -> Vec<J> {
    let mut out: Vec<J> = Vec::new();
    for v in dedupe(&an.verdicts) {
        out.push(verdict_json(an, &v));
    }
    for (label, entry) in schema_rows(an) {
        let keys: serde_json::Map<String, J> = entry
            .iter()
            .map(|(k, e)| (k.clone(), json!({"always": e.always, "sites": e.sites, "alternatives": e.alternatives, "caveats": e.caveats, "alt_keys": e.alt_keys})))
            .collect();
        out.push(json!({"kind": "schema", "source": label, "keys": keys}));
    }
    for ((qual, ctx), (_, rows)) in &an.summaries {
        let returns: serde_json::Map<String, J> = rows.iter().map(|(o, r)| (an.obj(*o).label.clone(), J::String(r.to_string()))).collect();
        out.push(json!({"kind": "summary", "function": qual, "context": ctx, "returns": returns}));
    }
    out
}

/// Everything the golden harness needs, as one JSON document.
pub fn golden_json(an: &Analyzer) -> J {
    let verdicts: Vec<J> = dedupe(&an.verdicts).iter().map(|v| verdict_json(an, v)).collect();
    let mut schemas = serde_json::Map::new();
    for (label, entry) in schema_rows(an) {
        let keys: serde_json::Map<String, J> = entry.iter().map(|(k, e)| (k.clone(), json!({"always": e.always}))).collect();
        schemas.insert(label, J::Object(keys));
    }
    let rows: Vec<J> = an
        .row_checks
        .iter()
        .map(|rc| {
            let value = json!({"lits": rc.value.lits.iter().map(|k| key_repr(*k)).collect::<Vec<_>>(), "unknown": rc.value.unknown, "objects": rc.value.objects});
            let mut out = match &rc.row {
            None => json!({"line": rc.line, "name": rc.name, "row": J::Null, "value": value}),
            Some(r) => json!({
                "line": rc.line, "name": rc.name, "value": value,
                "row": {
                    "present": r.present.iter().map(key_repr).collect::<Vec<_>>(),
                    "definite": r.definite.iter().map(key_repr).collect::<Vec<_>>(),
                    "absent": r.absent.iter().map(key_repr).collect::<Vec<_>>(),
                    "closed": r.closed(),
                    "text": r.to_string(),
                    "details": r.details(),
                }
            }),
            };
            if let (Some(J::Object(t)), J::Object(o)) = (&rc.tensor, &mut out) {
                for (k, v) in t {
                    o.insert(k.clone(), v.clone());
                }
            }
            out
        })
        .collect();
    let note = |n: &crate::models::torch::ShapeNote| json!({"line": n.line, "chain": n.chain, "entry": n.entry, "text": n.text, "message": n.text});
    let requires: Vec<J> = an.shape_reqs.iter().map(note).collect();
    let shape_errors: Vec<J> = an.shape_errors.iter().map(note).collect();
    json!({"verdicts": verdicts, "schemas": schemas, "rows": rows, "notes": an.notes, "report": format_text(an, true, true), "requires": requires, "shape_errors": shape_errors})
}

fn row_json(r: &crate::domain::Row) -> J {
    let srcs: Vec<String> = {
        let mut v: Vec<String> = r.sources.iter().map(|(s, l)| format!("{}{}", source_label(s), crate::domain::lens_str(l))).collect();
        v.sort();
        v
    };
    let muts: Vec<String> = {
        let mut v: Vec<String> = r.mutators.iter().map(|m| m.to_string()).collect();
        v.sort();
        v
    };
    json!({
        "present": intern::sorted_by_str(r.present.iter()).iter().map(|k| key_repr(*k)).collect::<Vec<_>>(),
        "definite": intern::sorted_by_str(r.definite.iter()).iter().map(|k| key_repr(*k)).collect::<Vec<_>>(),
        "likely": intern::sorted_by_str(r.likely.iter()).iter().map(|k| key_repr(*k)).collect::<Vec<_>>(),
        "absent": intern::sorted_by_str(r.absent.iter()).iter().map(|k| key_repr(*k)).collect::<Vec<_>>(),
        "closed": r.closed(),
        "top": r.top,
        "symbolic": r.symbolic(),
        "sources": srcs,
        "source_ids": r.sources.iter().map(|(s, _)| source_label(s)).collect::<Vec<_>>(),
        "mutators": muts,
        "text": r.to_string(),
        "details": r.details(),
    })
}

/// The `--facts` document for editor integration: per (line, name) the value and the rows
/// of the objects it may point to, the verdicts, and the schema table.
/// A value as the user would write it, as far as it is known (spec §2.2): literals (`1`,
/// `"a"`, several as `1 | 2 | ?`), dicts with their keys and values (`{"a": 1, "b"?: ?, **?}`),
/// lists / tuples by position when known (`[1, 2]`, `(1,)`), else `list[X]` /
/// `tuple[X, ...]`, instances as `Cls(a=1, **?)`, functions / classes by name, `?` for
/// anything unknown (`**?`: other keys or attributes may exist).  Depth and width are
/// limited (`…` marks what is not shown; also a cycle).
/// How deep rendered values go before nested containers are cut to `[…]` / `{…}` / `Name(…)`
/// (`--value-depth N`; default 3).
pub static VALUE_DEPTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(3);

pub fn render_value(an: &Analyzer, state: &State, v: &Value, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let mut parts: Vec<String> = Vec::new();
    // literals
    let lits = crate::domain::kv_literals(&v.kv);
    if v.kv.is_literal() && !lits.is_empty() {
        let mut ls: Vec<String> = intern::sorted_by_str(lits.iter()).iter().map(|k| crate::domain::key_display(*k)).collect();
        if ls.len() > 4 {
            ls.truncate(4);
            ls.push("…".into());
        }
        parts.extend(ls);
    }
    for o in v.pts.iter() {
        parts.push(render_object(an, state, o, depth, seen));
    }
    // a part nothing is known about (data, a parameter, keys of something): `?` (not for a
    // placeholder's own key-value: the placeholder is printed already)
    let own_param = matches!(&v.kv, KeyVal::Param(ph) if v.pts.contains(*ph));
    if let Some(expr) = data_expr(&v.kv) {
        parts.push(expr);
    } else if !v.kv.is_literal() && !v.kv.is_bot() && !own_param {
        parts.push("?".into());
    }
    if parts.is_empty() {
        return "?".into();
    }
    parts.sort();
    parts.dedup();
    // the unknown part last: `1 | 2 | ?`
    if parts.len() > 1 && parts.iter().any(|p| p == "?") {
        parts.retain(|p| p != "?");
        parts.push("?".into());
    }
    parts.join(" | ")
}

fn render_object(an: &Analyzer, state: &State, o: ObjId, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let max_depth: u32 = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    const MAX_ITEMS: usize = 8;
    if !an.has_obj(o) {
        return "?".into();
    }
    let kind = an.kind(o);
    if seen.contains(&o) {
        // a cycle back to an object being printed
        return "…".into();
    }
    let deeper = depth + 1;
    match kind {
        Kind::Dict | Kind::Frame | Kind::Namespace => {
            let row = state.row(o, Ns::Item);
            if depth >= max_depth {
                return if row.present.is_empty() && row.closed() { "{}".into() } else { "{…}".into() };
            }
            render_record(an, state, o, Ns::Item, None, depth, seen)
        }
        Kind::List | Kind::Tuple | Kind::Set => {
            let (open, close, ty) = match kind {
                Kind::List => ("[", "]", "list"),
                Kind::Tuple => ("(", ")", "tuple"),
                _ => ("{", "}", "set"),
            };
            seen.push(o);
            let out = match an.obj(o).literal_len {
                // known positions: the elements in order
                Some(n) if kind != Kind::Set => {
                    if depth >= max_depth {
                        seen.pop();
                        return if n == 0 { format!("{}{}", open, close) } else { format!("{}…{}", open, close) };
                    }
                    let mut items: Vec<String> = (0..n.min(MAX_ITEMS)).map(|i| match state.slot_opt(o, Ns::Pos, intern::key_int(i as i64)) {
                        Some(sv) => render_value(an, state, &sv, deeper, seen),
                        None => "?".into(),
                    }).collect();
                    if n > MAX_ITEMS {
                        items.push("…".into());
                    }
                    if kind == Kind::Tuple && n == 1 {
                        format!("({},)", items[0])
                    } else {
                        format!("{}{}{}", open, items.join(", "), close)
                    }
                }
                // unknown length: what any element may be, in typing notation
                _ => {
                    let elem = state.slot_opt(o, Ns::Elem, STAR);
                    let ekv = state.elems.get(&o);
                    let known_empty = matches!(&ekv, Some(KeyVal::Lits(ks)) if ks.is_empty());
                    let mut ev = elem.unwrap_or_default();
                    if let Some(kv) = ekv {
                        if kv.is_literal() && ev.kv.is_bot() {
                            ev = Value::with_kv(ev.pts.clone(), kv);
                        }
                    }
                    let inner = if depth >= max_depth { "…".to_string() } else if ev.empty() { "?".to_string() } else { render_value(an, state, &ev, deeper, seen) };
                    if ev.empty() && known_empty {
                        format!("{}{}", open, close)
                    } else if kind == Kind::Tuple {
                        format!("tuple[{}, ...]", inner)
                    } else {
                        format!("{}[{}]", ty, inner)
                    }
                }
            };
            seen.pop();
            out
        }
        Kind::Instance => {
            let name = an.instance_class(o).and_then(|c| an.class_info_opt(c)).map(|ci| ci.name.clone()).unwrap_or_else(|| "object".into());
            if depth >= max_depth {
                return format!("{}(…)", name);
            }
            render_record(an, state, o, Ns::Attr, Some(&name), depth, seen)
        }
        Kind::Function => an.obj(o).label.trim_start_matches("def ").to_string(),
        Kind::Bound => {
            let l = an.obj(o).label.clone();
            let name = l.trim_start_matches("def ").split(" bound").next().unwrap_or(&l).to_string();
            format!("{} (bound)", name)
        }
        Kind::Class => an.obj(o).label.clone(),
        Kind::Module => an.obj(o).label.clone(),
        // an unknown named in the program's terms (`a`, `a(b)`, `cfg["db"]`), else `?`
        Kind::Unknown | Kind::Ph => an.symbol_for(o).cloned().unwrap_or_else(|| "?".into()),
        Kind::External => an.external_qual(o).trim_start_matches("builtins.").to_string(),
        _ => "?".into(),
    }
}

thread_local! {
    /// The function whose names expressions are printed for (`expr:` data from elsewhere —
    /// a callee's `1 + b` returned to its caller — prints as `?`).
    static RENDER_SCOPE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub fn render_scope() -> u32 {
    RENDER_SCOPE.with(|c| c.get())
}

/// Run `f` printing values as seen inside function `scope` (0: module level).
pub fn in_scope<R>(scope: u32, f: impl FnOnce() -> R) -> R {
    let prev = RENDER_SCOPE.with(|c| c.replace(scope));
    let r = f();
    RENDER_SCOPE.with(|c| c.set(prev));
    r
}

/// An unknown value computed by an expression rowan names it by (`1 + b`), when that
/// expression is in the scope being printed.
pub fn data_expr(kv: &KeyVal) -> Option<String> {
    let KeyVal::Data(s) = kv else { return None };
    let label = intern::str_get(*s);
    let rest = label.strip_prefix("expr:")?;
    let (scope, text) = rest.split_once(':')?;
    let here = RENDER_SCOPE.with(|c| c.get());
    (scope.parse::<u32>().ok()? == here).then(|| text.to_string())
}

/// The keys (`ns` Item: `{"a": 1, "b"?: ?, **?}`) or attributes (Attr: `Name(a=1, **?)`) of an
/// object, with their values.
fn render_record(an: &Analyzer, state: &State, o: ObjId, ns: Ns, name: Option<&str>, depth: u32, seen: &mut Vec<ObjId>) -> String {
    const MAX_ITEMS: usize = 8;
    let row = state.row(o, ns);
    let mark = |r: &Row, k: KeyId| -> &'static str {
        if r.definite.contains(k) {
            ""
        } else if r.likely.contains(k) {
            "~"
        } else {
            "?"
        }
    };
    seen.push(o);
    let mut items: Vec<String> = Vec::new();
    let keys = intern::sorted_by_str(row.present.iter());
    for k in keys.iter().take(MAX_ITEMS) {
        let val = match state.slot_opt(o, ns, *k) {
            Some(sv) => render_value(an, state, &sv, depth + 1, seen),
            None => "?".into(),
        };
        if ns == Ns::Attr {
            items.push(format!("{}{}={}", intern::key_py_str(*k), mark(&row, *k), val));
        } else {
            items.push(format!("{}{}: {}", crate::domain::key_display(*k), mark(&row, *k), val));
        }
    }
    if keys.len() > MAX_ITEMS {
        items.push("…".into());
    }
    if !row.closed() {
        items.push("**?".into());
    }
    seen.pop();
    match (ns, name) {
        (Ns::Attr, Some(n)) => format!("{}({})", n, items.join(", ")),
        (Ns::Attr, None) => format!("obj({})", items.join(", ")),
        _ => format!("{{{}}}", items.join(", ")),
    }
}

/// The contents (slots) of the objects a value reaches, within the printing depth.
pub fn reachable_slots(an: &Analyzer, state: &State, v: &Value) -> Vec<(ObjId, Ns, KeyId, Value)> {
    let max = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    let mut out: Vec<(ObjId, Ns, KeyId, Value)> = Vec::new();
    let mut seen: Vec<ObjId> = Vec::new();
    let mut work: Vec<(Value, u32)> = vec![(v.clone(), 0)];
    while let Some((val, d)) = work.pop() {
        if d > max || out.len() > 256 {
            continue;
        }
        for o in val.pts.iter() {
            if seen.contains(&o) || matches!(an.kind(o), Kind::Function | Kind::Class | Kind::Module | Kind::External) {
                continue;
            }
            seen.push(o);
            if matches!(an.kind(o), Kind::Unknown | Kind::Ph) {
                // unknowns: only their rows matter (`reachable_rows`)
                continue;
            }
            for ns in [Ns::Item, Ns::Attr, Ns::Pos, Ns::Elem] {
                for (k, sv) in state.obj_slots(o, ns) {
                    work.push((sv.clone(), d + 1));
                    out.push((o, ns, k, sv));
                }
            }
        }
    }
    out
}

/// The rows (keys / attributes) of every object a value reaches within the printing depth.
pub fn reachable_rows(an: &Analyzer, state: &State, v: &Value) -> Vec<(ObjId, Option<Row>, Option<Row>)> {
    let max = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    let mut out: Vec<(ObjId, Option<Row>, Option<Row>)> = Vec::new();
    let mut work: Vec<(Value, u32)> = vec![(v.clone(), 0)];
    while let Some((val, d)) = work.pop() {
        if d > max || out.len() > 128 {
            continue;
        }
        for o in val.pts.iter() {
            if out.iter().any(|(x, _, _)| *x == o) || matches!(an.kind(o), Kind::Function | Kind::Class | Kind::Module | Kind::External) {
                continue;
            }
            out.push((o, state.has_row(o, Ns::Item).then(|| state.row(o, Ns::Item)), state.has_row(o, Ns::Attr).then(|| state.row(o, Ns::Attr))));
            for ns in [Ns::Item, Ns::Attr, Ns::Pos, Ns::Elem] {
                for (_, sv) in state.obj_slots(o, ns) {
                    work.push((sv, d + 1));
                }
            }
        }
    }
    out
}

/// The symbols a value mentions (unknown objects printed by name) with what is known about
/// each: the keys / attributes seen on it, what an unknown call may return, else `?`.
pub fn value_legend(an: &Analyzer, state: &State, v: &Value) -> Vec<(String, String)> {
    fn walk(an: &Analyzer, state: &State, v: &Value, depth: u32, seen: &mut Vec<ObjId>, out: &mut Vec<(String, String)>) {
        if depth > VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed) || out.len() >= 8 {
            return;
        }
        if let Some(expr) = data_expr(&v.kv) {
            if !out.iter().any(|(s0, _)| *s0 == expr) {
                out.push((expr, "?".to_string()));
            }
        }
        for o in v.pts.iter() {
            if seen.contains(&o) {
                continue;
            }
            seen.push(o);
            match an.kind(o) {
                Kind::Unknown | Kind::Ph => {
                    let Some(sym) = an.symbol_for(o) else { continue };
                    if out.iter().any(|(s, _)| s == sym) {
                        continue;
                    }
                    let mut parts: Vec<String> = Vec::new();
                    let item = state.row(o, Ns::Item);
                    if !item.present.is_empty() {
                        parts.push(render_record(an, state, o, Ns::Item, None, depth, &mut Vec::new()));
                    }
                    let attr = state.row(o, Ns::Attr);
                    if !attr.present.is_empty() {
                        parts.push(render_record(an, state, o, Ns::Attr, None, depth, &mut Vec::new()));
                    }
                    if let Some((_, _, given)) = an.origins.get(&o) {
                        parts.extend(given.iter().cloned());
                    }
                    // keys seen on it describe it; objects an unknown call was given are what it
                    // may also be (besides anything else)
                    let given = an.origins.get(&o).map(|(_, _, g)| !g.is_empty()).unwrap_or(false);
                    let desc = if parts.is_empty() { "?".to_string() } else if given { format!("{} | ?", parts.join(" | ")) } else { parts.join(" | ") };
                    out.push((sym.clone(), desc));
                }
                Kind::Dict | Kind::Frame | Kind::Namespace => {
                    for (_, sv) in state.obj_slots(o, Ns::Item) {
                        walk(an, state, &sv, depth + 1, seen, out);
                    }
                }
                Kind::Instance => {
                    for (_, sv) in state.obj_slots(o, Ns::Attr) {
                        walk(an, state, &sv, depth + 1, seen, out);
                    }
                }
                Kind::List | Kind::Tuple | Kind::Set => {
                    for (_, sv) in state.obj_slots(o, Ns::Pos) {
                        walk(an, state, &sv, depth + 1, seen, out);
                    }
                    if let Some(e) = state.slot_opt(o, Ns::Elem, STAR) {
                        walk(an, state, &e, depth + 1, seen, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(an, state, v, 0, &mut Vec::new(), &mut out);
    // in the order the value mentions them
    let text = render_value(an, state, v, 0, &mut Vec::new());
    out.sort_by_key(|(s0, _)| text.find(s0.as_str()).unwrap_or(usize::MAX));
    out
}

pub fn facts_json(an: &Analyzer) -> J {
    let sch = schema_rows(an);
    let mut facts: Vec<J> = Vec::new();
    for ((line, name, symbolic), f) in &an.facts {
        let objects: Vec<J> = f
            .rows
            .iter()
            .map(|(oid, (item, attr))| {
                let o = an.obj(*oid);
                let origin = an.origins.get(oid).map(|(t, l, g)| json!({"text": t, "line": l, "given": g}));
                json!({
                    "id": oid,
                    "label": o.label,
                    "kind": o.kind.name(),
                    "item": item.as_ref().map(row_json),
                    "attr": attr.as_ref().map(row_json),
                    "origin": origin,
                    "symbol": an.symbols.get(oid).filter(|_| f.legend.iter().any(|(s0, _)| Some(s0) == an.symbols.get(oid))),
                })
            })
            .collect();
        let values: J = match &f.value.kv {
            crate::domain::KeyVal::Lit(k) => json!([key_repr(*k)]),
            crate::domain::KeyVal::Lits(ks) => json!(intern::sorted_by_str(ks.iter()).iter().map(|k| key_repr(*k)).collect::<Vec<_>>()),
            crate::domain::KeyVal::KeysOf(ko) => json!([format!("{}", crate::domain::KeyVal::KeysOf(ko.clone()))]),
            crate::domain::KeyVal::Data(_) => json!(["<data>"]),
            crate::domain::KeyVal::Param(_) => json!(["<parameter>"]),
            crate::domain::KeyVal::Bot => json!([]),
        };
        // requirements recorded on the open sources of these objects
        let mut required: serde_json::Map<String, J> = serde_json::Map::new();
        for (_, (item, attr)) in &f.rows {
            for r in [item, attr].into_iter().flatten() {
                for (src, _) in r.sources.iter() {
                    let label = source_label(src);
                    if let Some(entry) = sch.get(&label) {
                        let keys: serde_json::Map<String, J> = entry.iter().map(|(k, e)| (k.clone(), json!(e.always))).collect();
                        required.insert(label, J::Object(keys));
                    }
                }
            }
        }
        facts.push(json!({"line": line, "name": name.to_string(), "symbolic": symbolic, "values": values, "text": f.texts.join(" | "), "legend": f.legend.iter().map(|(s0, d)| json!([s0, d])).collect::<Vec<_>>(), "objects": objects, "required": required}));
    }
    let verdicts: Vec<J> = dedupe(&an.verdicts).iter().map(|v| verdict_json(an, v)).collect();
    let mut schemas = serde_json::Map::new();
    for (label, entry) in sch {
        let keys: serde_json::Map<String, J> = entry.iter().map(|(k, e)| (k.clone(), json!({"always": e.always, "sites": e.sites}))).collect();
        schemas.insert(label, J::Object(keys));
    }
    json!({"file": an.filename, "facts": facts, "verdicts": verdicts, "schemas": schemas, "notes": an.notes})
}
