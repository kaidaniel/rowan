//! Reports: per-access combination across targets, schema table, text / JSON (port of
//! `rowan/report.py`).

use indexmap::IndexMap;
use serde_json::{json, Value as J};

use crate::domain::{source_is_data, source_label, Ns, Tier};
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
    let vs: Vec<&Verdict> = all.iter().filter(|v| !matches!(v.tier, Tier::Proven | Tier::Map)).collect();
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
            lines.push("== module-level records at exit ==".into());
            let m = an.main_module;
            let mut names: Vec<(String, crate::domain::Value)> = st.obj_slots(m, Ns::Attr).into_iter().map(|(k, v)| (intern::key_py_str(k), v)).collect();
            names.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, v) in names {
                for oid in v.pts.iter() {
                    match an.kind(oid) {
                        Kind::Dict | Kind::Frame | Kind::Unknown | Kind::Namespace => lines.push(format!("  {} = {}: {}", name, an.obj(oid).label, st.row(oid, Ns::Item))),
                        Kind::Instance => lines.push(format!("  {} = {}: {}", name, an.obj(oid).label, st.row(oid, Ns::Attr).compact(true))),
                        _ => {}
                    }
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
        .map(|rc| match &rc.row {
            None => json!({"line": rc.line, "name": rc.name, "row": J::Null}),
            Some(r) => json!({
                "line": rc.line, "name": rc.name,
                "row": {
                    "present": r.present.iter().map(key_repr).collect::<Vec<_>>(),
                    "definite": r.definite.iter().map(key_repr).collect::<Vec<_>>(),
                    "absent": r.absent.iter().map(key_repr).collect::<Vec<_>>(),
                    "closed": r.closed(),
                    "text": r.to_string(),
                    "details": r.details(),
                }
            }),
        })
        .collect();
    json!({"verdicts": verdicts, "schemas": schemas, "rows": rows, "notes": an.notes, "report": format_text(an, true, true)})
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
pub fn facts_json(an: &Analyzer) -> J {
    let sch = schema_rows(an);
    let mut facts: Vec<J> = Vec::new();
    for ((line, name, symbolic), f) in &an.facts {
        let objects: Vec<J> = f
            .rows
            .iter()
            .map(|(oid, (item, attr))| {
                let o = an.obj(*oid);
                json!({
                    "id": oid,
                    "label": o.label,
                    "kind": o.kind.name(),
                    "item": item.as_ref().map(row_json),
                    "attr": attr.as_ref().map(row_json),
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
        facts.push(json!({"line": line, "name": name.to_string(), "symbolic": symbolic, "values": values, "objects": objects, "required": required}));
    }
    let verdicts: Vec<J> = dedupe(&an.verdicts).iter().map(|v| verdict_json(an, v)).collect();
    let mut schemas = serde_json::Map::new();
    for (label, entry) in sch {
        let keys: serde_json::Map<String, J> = entry.iter().map(|(k, e)| (k.clone(), json!({"always": e.always, "sites": e.sites}))).collect();
        schemas.insert(label, J::Object(keys));
    }
    json!({"file": an.filename, "facts": facts, "verdicts": verdicts, "schemas": schemas, "notes": an.notes})
}
