//! Reports: per-access combination across targets, schema table, text / JSON (port of
//! `rowan/report.py`).

use indexmap::IndexMap;
use serde_json::{json, Value as J};

use crate::domain::{source_is_data, source_label, KeyVal, Ns, ObjId, Row, SourceId, Tier, Value};
use crate::intern::{KeyId, STAR};
use crate::state::State;
use crate::intern::{self, key_repr};
use crate::interp::{Analyzer, Verdict};
use crate::state::Kind;

/// One verdict per (access site, key, call chain): per-target verdicts are combined (§10).
pub fn dedupe(an: &Analyzer, verdicts: &[Verdict]) -> Vec<Verdict> {
    let mut groups: IndexMap<(String, u32, u32, String, Vec<u32>, Ns, String, bool), Vec<&Verdict>> = IndexMap::new();
    for v in verdicts {
        groups.entry((v.file.clone(), v.line, v.col, key_repr(v.key), v.chain.clone(), v.ns, v.text.clone(), v.guarded)).or_default().push(v);
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
        // (an object the access fails on in some runs of the line — an iteration, a call — and
        // finds the key in others: it may fail)
        let mut mixed: Vec<Verdict> = Vec::new();
        for (o, v) in by_obj.iter() {
            if matches!(v.tier, Tier::Missing | Tier::LikelyMissing) && vs.iter().any(|w| w.oid == *o && w.tier.rank() < Tier::Possibly.rank()) {
                mixed.push(Verdict { tier: Tier::Possibly, note: "is missing on some of the times this line runs".into(), ..(*v).clone() });
            }
        }
        for m in mixed.iter() {
            by_obj.insert(m.oid, m);
        }
        let names: IndexMap<u32, String> = by_obj.iter().map(|(o, v)| (*o, v.obj.clone())).collect();
        let mut tiers: Vec<Tier> = by_obj.values().map(|v| v.tier).collect();
        tiers.sort();
        tiers.dedup();
        // (with an object rowan knows nothing of, the group says nothing — but where the
        // access fails on another object: it may fail)
        if tiers.contains(&Tier::Unknown) && !tiers.contains(&Tier::Missing) {
            continue;
        }
        let first = vs[0];
        if tiers.len() == 1 {
            let v = by_obj.values().next().unwrap();
            out.push(Verdict { obj: object_list(names.values()), ..(*v).clone() });
            continue;
        }
        // (per tier, the objects it holds for)
        let mut by_tier: IndexMap<Tier, Vec<String>> = IndexMap::new();
        for (o, v) in by_obj.iter() {
            by_tier.entry(v.tier).or_default().push(object_text(an, &names[o]));
        }
        by_tier.sort_keys();
        // (where it fails: the objects that lack the key, then those that may)
        let word = |t: &Tier| match t {
            Tier::Proven | Tier::Likely | Tier::Input | Tier::Map | Tier::Unknown => None,
            Tier::Possibly | Tier::DataKey => Some("maybe missing on"),
            _ => Some("missing on"),
        };
        let breakdown: Vec<String> = by_tier.iter().rev().filter_map(|(t, ns)| word(t).map(|w| format!("{} {}", w, object_list(ns.iter())))).collect();
        let tier = if tiers.iter().all(|t| matches!(t, Tier::Proven | Tier::Likely)) {
            Tier::Likely
        } else if tiers.iter().all(|t| matches!(t, Tier::Input | Tier::Proven | Tier::Likely)) {
            Tier::Input
        } else if tiers.iter().all(|t| matches!(t, Tier::Input | Tier::Proven | Tier::Likely | Tier::LikelyMissing)) {
            // (missing on some objects only unless code rowan cannot see set it: as hedged)
            Tier::LikelyMissing
        } else {
            Tier::Possibly
        };
        let mut provs: Vec<SourceId> = by_obj.values().flat_map(|v| v.providers.clone()).collect();
        provs.sort();
        provs.dedup();
        out.push(Verdict {
            line: first.line,
            col: first.col,
            key: first.key,
            tier,
            obj: object_list(names.values()),
            chain: first.chain.clone(),
            // (no object lacks it: the note of the least sure one — unknown code ran, an input)
            note: if breakdown.is_empty() { by_obj.values().max_by_key(|v| v.tier.rank()).map(|v| v.note.clone()).unwrap_or_default() } else { format!("is not on every object this may be ({})", breakdown.join("; ")) },
            providers: provs,
            mutators: first.mutators.clone(),
            ns: first.ns,
            text: first.text.clone(),
            oid: first.oid,
            file: first.file.clone(),
            guarded: first.guarded,
        });
    }
    // the same finding reached through many call chains: keep the shortest few chains
    let mut by_finding: IndexMap<(String, u32, u32, String, Ns, String, Tier, String, String, bool), Vec<Verdict>> = IndexMap::new();
    for v in out {
        by_finding.entry((v.file.clone(), v.line, v.col, key_repr(v.key), v.ns, v.text.clone(), v.tier, v.obj.clone(), v.note.clone(), v.guarded)).or_default().push(v);
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

/// An object for people: `dict@L4` as it is, an unknown call's result by the call.
fn object_text(an: &Analyzer, label: &str) -> String {
    if let Some(site) = label.strip_prefix("result of ") {
        return format!("the result of {}", site_text(an, site));
    }
    if let Some(x) = label.strip_prefix("param ") {
        return format!("argument `{}`", x);
    }
    label.to_string()
}

/// Objects named by where they were made, each once with how many there are (`dict@L4 ×2`),
/// the first few only (`… 5 more`).
fn object_list<'a>(names: impl Iterator<Item = &'a String>) -> String {
    const SHOWN: usize = 3;
    let mut counts: IndexMap<&str, usize> = IndexMap::new();
    for n in names {
        *counts.entry(n.as_str()).or_default() += 1;
    }
    counts.sort_keys();
    let mut parts: Vec<String> = counts.iter().take(SHOWN).map(|(n, c)| if *c > 1 { format!("{} ×{}", n, c) } else { n.to_string() }).collect();
    if counts.len() > SHOWN {
        parts.push(format!("… {} more", counts.len() - SHOWN));
    }
    parts.join(", ")
}

/// How many of the objects a name may hold the facts document describes (the rest: a count).
pub const MAX_FACT_OBJECTS: usize = 16;

/// How many distinct call chains one finding is reported under (the shortest ones).
pub const MAX_CHAINS_PER_FINDING: usize = 8;

/// What the program reads of each input, by its label: {key -> read on every path}.
pub fn schema_rows(an: &Analyzer) -> IndexMap<String, IndexMap<String, J>> {
    let mut out: IndexMap<String, IndexMap<String, J>> = IndexMap::new();
    let Some(st) = &an.findings.final_state else { return out };
    for (src, d) in st.reqs.items().into_iter().filter(|(s, _)| !source_is_data(*s)) {
        let entry = out.entry(source_label(src)).or_default();
        for (k, info) in d.iter() {
            let mut j = json!({"always": info.always});
            if !info.only_if.is_empty() {
                j["only_if"] = J::String(only_if_text(&info.only_if));
            }
            entry.insert(key_repr(*k), j);
        }
    }
    out.sort_keys();
    out.values_mut().for_each(|e| e.sort_keys());
    out
}

/// The text report: what may fail, what the code needs of its inputs and arguments, what
/// functions return, the module's values, the limits of the analysis, and a legend of the
/// marks used. Written for people who know Python, not static analysis.
pub fn format_text(an: &Analyzer, show_rows: bool, show_summaries: bool) -> String {
    fn section(lines: &mut Vec<String>, title: &str, body: Vec<String>) {
        if !body.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!("== {} ==", title));
            lines.extend(body);
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let all = reported(an);
    let mut problems = problem_lines(an, &all);
    let none = problems.is_empty();
    if none {
        problems.push("  (none found)".into());
    }
    let oks = all.iter().filter(|v| v.tier == Tier::Proven).count();
    let likely = all.iter().filter(|v| v.tier == Tier::Likely).count();
    // ("(3 other accesses always succeed; 1 more unless code rowan cannot see removed the key)")
    let n_acc = |n: usize| format!("{} {}access{}", n, if none { "" } else { "other " }, if n == 1 { "" } else { "es" });
    let ok = (oks > 0).then(|| format!("{} always succeed{}", n_acc(oks), if oks == 1 { "s" } else { "" }));
    let lk = (likely > 0).then(|| format!("{} unless code rowan cannot see removed the key", if oks > 0 { format!("{} more", likely) } else { format!("{} succeed{}", n_acc(likely), if likely == 1 { "s" } else { "" }) }));
    let counts: Vec<String> = ok.into_iter().chain(lk).collect();
    if !counts.is_empty() {
        problems.push(format!("  ({})", counts.join("; ")));
    }
    section(&mut lines, "accesses that may fail", problems);
    // tensor shapes: definite mismatches, then what each line needs of its inputs
    if an.m.torch_on {
        let mut shapes: Vec<String> = Vec::new();
        let mut errs: Vec<&crate::models::torch::ShapeNote> = an.m.shape_errors.iter().collect();
        // (a line of another module is named with its file: `pkg/layers.py:L12`)
        let at = |n: &crate::models::torch::ShapeNote| match an.program.rel_file(n.module) {
            f if f.is_empty() => format!("L{:<5}", n.line),
            f => format!("{}:L{}", f, n.line),
        };
        errs.sort_by_key(|e| (e.module != an.program.main_idx, e.module, e.line, e.chain.len()));
        errs.dedup_by(|a, b| (a.module, a.line, &a.text) == (b.module, b.line, &b.text));
        shapes.extend(errs.iter().map(|e| format!("  {} ✗  shape mismatch: {}{}", at(e), e.text, via_text(&e.chain, 0))));
        let mut reqs: Vec<(bool, u32, u32, String, String)> = an.m.shape_reqs.iter().filter(|r| r.chain.is_empty()).map(|r| (r.module != an.program.main_idx, r.module, r.line, at(r), r.text.clone())).collect();
        reqs.sort();
        reqs.dedup();
        shapes.extend(reqs.iter().map(|(_, _, _, at, text)| format!("  {}    needs {}", at, text)));
        if shapes.is_empty() {
            shapes.push("  (no mismatches, no requirements)".into());
        }
        section(&mut lines, "tensor shapes", shapes);
    }
    let (inputs, contracts) = needs_text(an);
    section(&mut lines, "what the code reads from its inputs", inputs);
    section(&mut lines, "what each function needs from its arguments", contracts);
    if show_summaries {
        section(&mut lines, "what functions return", result_lines(an));
    }
    if let Some(st) = an.findings.final_state.as_ref().filter(|_| show_rows) {
        let mut names: Vec<(String, crate::domain::Value)> = st.obj_slots(an.program.main_module, Ns::Attr).into_iter().map(|(k, v)| (intern::key_py_str(k), v)).collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        // definitions (functions, classes, imported modules) are not values worth listing;
        // one line each, cut at 150 characters (the hover has the rest)
        let vals = names
            .into_iter()
            .filter(|(_, v)| v.pts.is_empty() || !v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::Function | Kind::Class | Kind::Module | Kind::External)))
            .map(|(name, v)| format!("  {} = {}", name, render_value(an, st, &v, 0, &mut Vec::new())))
            .map(|l| if l.chars().count() > 150 { format!("{}…", l.chars().take(149).collect::<String>()) } else { l })
            .collect();
        section(&mut lines, "module-level variables at the end", vals);
    }
    section(&mut lines, "limits of this analysis", limit_lines(an));
    let legend = legend(&lines.join("\n"));
    section(&mut lines, "legend", legend);
    lines.join("\n")
}

/// The mark of a verdict in reports: one per kind of finding.
fn tier_mark(t: Tier) -> &'static str {
    match t {
        Tier::DataKey => "⚠",
        t => t.symbol(),
    }
}

/// One line per access that may fail (✗, ⚠, ⚠~): the reasons in words; an access reached
/// through several calls once, with the shortest call path and how many others there are.
fn problem_lines(an: &Analyzer, vs: &[Verdict]) -> Vec<String> {
    // (per access and key: the most severe finding, with the call paths of all of them)
    let mut groups: IndexMap<(String, u32, u32, String, String), Vec<&Verdict>> = IndexMap::new();
    for v in vs.iter().filter(|v| matches!(v.tier, Tier::Missing | Tier::Possibly | Tier::LikelyMissing | Tier::DataKey)) {
        groups.entry((v.file.clone(), v.line, v.col, v.text.clone(), key_repr(v.key))).or_default().push(v);
    }
    let mut groups: IndexMap<(String, u32, u32, String, String), Vec<&Verdict>> = groups
        .into_iter()
        .map(|((f, l, c, t, _), mut vs)| {
            vs.sort_by_key(|v| (std::cmp::Reverse(v.tier.rank()), v.chain.len()));
            let mm = format!("{}\u{0}{}", tier_mark(vs[0].tier), verdict_message(an, vs[0]));
            ((f, l, c, t, mm), vs)
        })
        .collect();
    // (grouped by file: the main file first, then the others under their names)
    groups.sort_by(|a, _, b, _| (!a.0.is_empty(), &a.0, a.1, a.2).cmp(&(!b.0.is_empty(), &b.0, b.1, b.2)));
    // (the same access written several times on a line, with the same finding: one row, `×2`)
    let mut rows: IndexMap<(String, u32, String, String), (usize, Vec<&Verdict>)> = IndexMap::new();
    for ((f, l, _, t, mm), vs) in groups {
        let e = rows.entry((f, l, t, mm)).or_default();
        e.0 += 1;
        e.1.extend(vs);
    }
    let rows: Vec<((String, u32, String, String), Vec<&Verdict>)> = rows.into_iter().map(|((f, l, t, mm), (n, vs))| ((f, l, if n > 1 { format!("{} ×{}", t, n) } else { t }, mm), vs)).collect();
    let width = rows.iter().map(|(k, _)| k.2.chars().count()).max().unwrap_or(0).min(36);
    let mut out = Vec::new();
    let mut file_now = String::new();
    for ((file, line, text, mm), vs) in rows {
        if file != file_now {
            out.push(format!("  {}:", file));
            file_now = file;
        }
        let (mark, msg) = mm.split_once('\u{0}').unwrap();
        // (reached directly: no call path to show)
        let mut chains: Vec<&Vec<u32>> = vs.iter().map(|v| &v.chain).collect();
        chains.sort();
        chains.dedup();
        let via = if vs.iter().any(|v| v.chain.is_empty()) { String::new() } else { via_text(&vs[0].chain, chains.len() - 1) };
        let text = if text.chars().count() > width { format!("{}…", text.chars().take(width - 1).collect::<String>()) } else { text };
        let indent = if file_now.is_empty() { "  " } else { "    " };
        out.push(format!("{}{:<5} {:<2} {:<w$}  {}{}", indent, format!("L{}", line), mark, text, msg, via, w = width));
    }
    out
}

/// `  via L5→L9 (+2 more)`: the calls (by line) through which an access was reached.
fn via_text(chain: &[u32], more: usize) -> String {
    if chain.is_empty() {
        return String::new();
    }
    let mut path: Vec<String> = chain.iter().map(|c| format!("L{}", c)).collect();
    // (a long path: its first call and the last two)
    if path.len() > 4 {
        path = vec![path[0].clone(), "…".into(), path[path.len() - 2].clone(), path[path.len() - 1].clone()];
    }
    let more = if more > 0 { format!(" (+{} more)", more) } else { String::new() };
    format!("  via {}{}", path.join("→"), more)
}

/// What a verdict says, in words: `key "role" is only set on some paths`.
pub fn verdict_message(an: &Analyzer, v: &Verdict) -> String {
    let what = match v.ns {
        Ns::Attr => format!("attribute .{}", intern::key_py_str(v.key)),
        Ns::Name => format!("name {}", intern::key_py_str(v.key)),
        Ns::Pos => format!("position {}", key_repr(v.key)),
        _ if v.key == STAR => "an element".to_string(),
        // (a key computed from data: no name to show)
        _ if v.tier == Tier::DataKey => "the key".to_string(),
        _ if an.heap.has_obj(v.oid) && an.heap.kind(v.oid) == Kind::Frame => format!("column {}", crate::domain::key_display(v.key)),
        // (an index or a dict key alike: `xs[0]`)
        _ if intern::key_as_int(v.key).is_some() => format!("item {}", key_repr(v.key)),
        _ => format!("key {}", crate::domain::key_display(v.key)),
    };
    let unknown_code = || code_list(an, &v.mutators);
    // (the other notes are the reason in words: `is only set on some paths`)
    match v.tier {
        Tier::Proven if v.note == crate::domain::FOUND_BEFORE => format!("{} is there: an earlier read or test on this path found it", what),
        Tier::Proven => format!("{} is always there", what),
        Tier::Likely => format!("{} is there, unless {} removed it", what, unknown_code()),
        Tier::Input => format!("{} must be provided by {}", what, v.providers.iter().map(|p| source_id_text(an, *p)).collect::<Vec<_>>().join(" or ")),
        Tier::LikelyMissing => format!("{} is never set, unless {} added it", what, unknown_code()),
        _ if v.note.is_empty() => format!("{} may be missing", what),
        _ => format!("{} {}", what, v.note),
    }
}

/// Unknown code by where it ran: "`f(d)` (line 3)", the first two and how many more.
fn code_list(an: &Analyzer, sites: &[String]) -> String {
    if sites.is_empty() {
        return "code rowan cannot see".into();
    }
    let mut names: Vec<String> = Vec::new();
    for s0 in sites {
        let t = site_text(an, s0);
        if !names.contains(&t) {
            names.push(t);
        }
    }
    names.sort_by_key(|n| n == "other calls");
    let n = names.len();
    let mut out = names.into_iter().take(2).collect::<Vec<_>>().join(" or ");
    if n > 2 {
        out += &format!(" (or {} other call{})", n - 2, if n == 3 { "" } else { "s" });
    }
    out
}

/// A place unknown values or changes come from, for people: an unknown call by its text
/// (`` `mo.group(0)` (line 497) ``), a parameter (`` argument `x` of `f` ``), an input
/// (`` `json.loads(…)` (line 6) ``).
pub fn site_text(an: &Analyzer, site: &str) -> String {
    let in_file = |m: u32, line: u32| {
        let f = an.program.rel_file(m);
        if f.is_empty() { format!("line {}", line) } else { format!("{} line {}", f, line) }
    };
    if let Some((text, m, line)) = an.names.site_names.get(&intern::s(site)) {
        let t: String = if text.chars().count() > 50 { format!("{}…", text.chars().take(49).collect::<String>()) } else { text.clone() };
        // (a call of the program's own code that rowan no longer follows: say why it is unknown)
        let not_followed = if an.names.skipped_sites.contains(&intern::s(site)) { ", a call rowan no longer follows" } else { "" };
        return format!("`{}` ({}{})", t, in_file(*m, *line), not_followed);
    }
    if site == "program@" {
        // (on demand: what the program's functions may have changed before a call)
        return "the program's functions called before".into();
    }
    if site == "…" {
        // (a row names its first few unknown calls, then this)
        return "other calls".into();
    }
    let clean = |q: &str| q.trim_start_matches("class ").trim_start_matches("builtins.").to_string();
    if let Some(q) = site.strip_prefix("recursion@") {
        return format!("the recursion in `{}`", clean(q));
    }
    if let Some(q) = site.strip_prefix("consumer@") {
        return format!("the code that takes the items of `{}`", clean(q));
    }
    if let Some(l) = site.strip_prefix("del@L") {
        return format!("`del` (line {})", l);
    }
    if let Some((f, x)) = site.strip_prefix("param ").and_then(|r| r.rsplit_once('.')) {
        return format!("argument `{}` of `{}`", x, clean(f));
    }
    if let Some(x) = site.strip_prefix("name ") {
        return format!("`{}` (not defined in this file)", x);
    }
    if let Some((q, l)) = site.rsplit_once("@L").filter(|(_, l)| l.chars().all(|c| c.is_ascii_digit())) {
        return format!("`{}(…)` (line {})", clean(q), l);
    }
    format!("`{}`", site)
}

/// An input for people, by id: its site and the path below it, each step written as the
/// code read it (`argument `items[*].sku` of `f``).
fn source_id_text(an: &Analyzer, src: SourceId) -> String {
    let (site, kind, path) = intern::with_source(src, |s| (s.site.clone(), s.kind, s.path.clone()));
    let t = site_text(an, &site);
    let Some(st) = an.findings.final_state.as_ref().filter(|_| !path.is_empty()) else { return t };
    let steps = path_steps(st, &site, kind, 0, &path).map(|p| p.0).unwrap_or_else(|| "…".into());
    match t.find("` ").or_else(|| t.ends_with('`').then(|| t.len() - 1)) {
        Some(i) if t[..i].contains('`') => format!("{}{}{}", &t[..i], steps, &t[i..]),
        _ => format!("{} {}", t, steps),
    }
}

/// The access path of `path[from..]` below an input (`.sku`, `["db"]`, `[*]`), each step as
/// the parent's requirement says it was read; whether a step is an attribute; None for a
/// path through what a method returned (`df.groupby(…)["amount"]`: a step the code called,
/// with no requirement of its own).
/// Also: whether every step is read on every path, and the tests of the parameters the steps'
/// reads are under (a path below is read only where the steps to it are).
fn path_steps(st: &State, site: &std::sync::Arc<str>, kind: crate::domain::SourceKind, from: usize, path: &[KeyId]) -> Option<(String, bool, bool, intern::IdSet<intern::StrId>)> {
    let mut out = String::new();
    let mut attr_step = false;
    let (mut always, mut only_if) = (true, intern::IdSet::new());
    for i in from..path.len() {
        let parent = intern::source(crate::domain::Source { site: site.clone(), kind, path: path[..i].to_vec() });
        let attr = match st.reqs.get(&parent, &path[i]) {
            Some(r) => {
                always &= r.always;
                only_if = only_if.union(&r.only_if);
                r.attr
            }
            None if path[i] != STAR && intern::key_as_str(path[i]).is_some() => return None,
            None => false,
        };
        attr_step |= attr;
        out += &step_text(path[i], attr);
    }
    Some((out, attr_step, always, only_if))
}

/// Is an unknown call's result an input worth listing (a library function the program
/// calls: `requests.get`, `read_csv`), not a value rowan lost track of (`?`, `unknown.strip`,
/// a call of a call's result, a function past its budget)?
fn named_call(site: &str) -> bool {
    let q = site.rsplit_once("@L").map(|(q, _)| q).unwrap_or(site);
    let first = q.split('.').next().unwrap_or("");
    !q.is_empty() && !["unknown", "builtins", "class", "view", "self", "cls"].contains(&first) && q.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

/// What the code needs of its inputs and of each function's arguments, as Python access
/// paths: (inputs, functions).
fn needs_text(an: &Analyzer) -> (Vec<String>, Vec<String>) {
    use crate::domain::SourceKind;
    let Some(st) = &an.findings.final_state else { return (vec![], vec![]) };
    // (functions of the program that rowan did not follow are not inputs)
    let program: std::collections::HashSet<String> = (0..an.heap.len() as ObjId).filter_map(|o| an.heap.func_info_opt(o)).map(|fi| fi.py_qualname()).collect();
    // (an input made in another module: its file, from the reads that needed it)
    let mut files: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for v in an.findings.verdicts().iter().filter(|v| !v.file.is_empty()) {
        for p in &v.providers {
            files.entry(intern::with_source(*p, |s| s.site.to_string())).or_insert_with(|| v.file.clone());
        }
    }
    let mut roots: Vec<(bool, String, SourceId, String)> = Vec::new();
    for (src, _) in st.reqs.items() {
        let (site, kind) = intern::with_source(src, |s| (s.site.to_string(), s.kind));
        let (param, header, base) = match kind {
            SourceKind::Param => match site.strip_prefix("param ").and_then(|r| r.rsplit_once('.')) {
                Some((f, x)) => (true, f.trim_start_matches("class ").to_string(), x.to_string()),
                None => continue,
            },
            SourceKind::External | SourceKind::Name => {
                let t = site_text(an, &site).replace('`', "");
                (false, match files.get(&site) { Some(f) => t.replace("(line ", &format!("({} line ", f)), None => t }, String::new())
            }
            SourceKind::Call if named_call(&site) && !program.contains(site.rsplit_once("@L").map(|(q, _)| q).unwrap_or(&site)) => (false, site_text(an, &site).replace('`', ""), String::new()),
            _ => continue,
        };
        let root = crate::domain::Source::new(&site, kind);
        if !roots.iter().any(|r| r.2 == root) {
            roots.push((param, header, root, base));
        }
    }
    roots.sort_by(|a, b| (&a.1, &a.3).cmp(&(&b.1, &b.3)));
    let mut groups: IndexMap<(bool, String), Vec<Need>> = IndexMap::new();
    for (param, header, root, base) in roots {
        // (attributes of what a library call returned are its API, not data the code needs)
        groups.entry((param, header)).or_default().extend(reads_below(st, root, &base, !param));
    }
    let (mut inputs, mut funcs) = (Vec::new(), Vec::new());
    let width = groups.keys().map(|(_, h)| h.chars().count()).max().unwrap_or(0).min(30);
    for ((param, header), needs) in groups {
        if needs.is_empty() {
            continue;
        }
        // (always, sometimes, then a line per test of the parameters the reads are under)
        let mut lines: Vec<(String, Vec<&str>)> = ["always:", "sometimes:"].iter().map(|l| (l.to_string(), needs.iter().filter(|n| n.always == (*l == "always:") && n.only_if.is_none()).map(|n| n.path.as_str()).collect())).collect();
        for n in needs.iter().filter(|n| n.only_if.is_some()) {
            let label = format!("only if {}:", n.only_if.as_deref().unwrap_or(""));
            match lines.iter_mut().find(|(l, _)| *l == label) {
                Some((_, ps)) => ps.push(n.path.as_str()),
                None => lines.push((label, vec![n.path.as_str()])),
            }
        }
        let out = if param { &mut funcs } else { &mut inputs };
        let mut head = format!("  {:<w$}", header, w = width);
        if header.chars().count() > width {
            out.push(format!("  {}", header));
            head = format!("  {:<w$}", "", w = width);
        }
        let pad = format!("  {:<w$}", "", w = width);
        for (i, (label, ps)) in lines.iter().filter(|(_, ps)| !ps.is_empty()).enumerate() {
            out.push(format!("{}  {:<10} {}", if i == 0 { &head } else { &pad }, label, index_runs(ps).join(", ")));
        }
    }
    (inputs, funcs)
}

/// Access paths with runs of three or more consecutive indexes of one path written once:
/// `t[0], t[1], t[2]` as `t[0..2]`.
fn index_runs(paths: &[&str]) -> Vec<String> {
    let split = |p: &str| -> Option<(String, i64)> {
        let (pre, idx) = p.strip_suffix(']')?.rsplit_once('[')?;
        Some((pre.to_string(), idx.parse().ok()?))
    };
    let mut out: Vec<String> = Vec::new();
    let mut done: Vec<String> = Vec::new();
    for p in paths {
        let Some((pre, _)) = split(p) else {
            out.push(p.to_string());
            continue;
        };
        if done.contains(&pre) {
            continue;
        }
        let mut idx: Vec<i64> = paths.iter().filter_map(|q| split(q)).filter(|(q, _)| *q == pre).map(|(_, i)| i).collect();
        idx.sort_unstable();
        idx.dedup();
        let mut i = 0;
        while i < idx.len() {
            let mut j = i;
            while j + 1 < idx.len() && idx[j + 1] == idx[j] + 1 {
                j += 1;
            }
            if j - i >= 2 {
                out.push(format!("{}[{}..{}]", pre, idx[i], idx[j]));
            } else {
                out.extend(idx[i..=j].iter().map(|n| format!("{}[{}]", pre, n)));
            }
            i = j + 1;
        }
        done.push(pre);
    }
    out
}

/// What the code reads of input `src` and of what lies below it, as access paths from
/// `base` (`row["id"]`, `items[*].sku`): (path, read on every path), the longest paths only.
/// `data_only`: keys and elements, not attributes.
pub fn reads_below(st: &State, src: SourceId, base: &str, data_only: bool) -> Vec<Need> {
    let (site, kind, top) = intern::with_source(src, |s| (s.site.clone(), s.kind, s.path.clone()));
    let mut needs: Vec<Need> = Vec::new();
    for (s2, d) in st.reqs.items() {
        let (site2, kind2, path) = intern::with_source(s2, |s| (s.site.clone(), s.kind, s.path.clone()));
        if site2 != site || kind2 != kind || !path.starts_with(&top) {
            continue;
        }
        let Some((prefix, attr_step, steps_always, steps_only_if)) = path_steps(st, &site, kind, top.len(), &path) else { continue };
        if data_only && attr_step {
            continue;
        }
        for (k, info) in d.iter() {
            if data_only && info.attr {
                continue;
            }
            // (a key unknown code or another input may supply is needed only sometimes)
            let always = info.always && steps_always && info.caveats.is_empty() && info.alternatives.is_empty();
            let info_only_if = info.only_if.union(&steps_only_if);
            // (a group of alternatives reads the same from any of its keys: in one order)
            let mut alts: Vec<String> = std::iter::once(*k).chain(info.alt_keys.iter()).map(|a| format!("{}{}{}", base, prefix, step_text(a, info.attr))).collect();
            alts.sort();
            alts.dedup();
            let only_if = (!always && !info_only_if.is_empty()).then(|| only_if_text(&info_only_if));
            needs.push(Need { path: alts.join(" or "), always, only_if });
        }
    }
    needs.sort_by(|a, b| a.path.cmp(&b.path));
    needs.dedup_by(|a, b| a.path == b.path && {
        b.only_if = if b.only_if == a.only_if { b.only_if.take() } else { None };
        b.always = b.always && a.always;
        true
    });
    // (a path read below another needs that one too: only the longer is listed)
    let all: Vec<String> = needs.iter().map(|n| n.path.clone()).collect();
    needs.retain(|n| !all.iter().any(|q| q.len() > n.path.len() && q.starts_with(n.path.as_str()) && matches!(q.as_bytes()[n.path.len()], b'[' | b'.')));
    needs
}

/// What the code reads of an input: an access path, whether on every path, else the tests of
/// the function's parameters every path that reads it passed (`admin`).
#[derive(Clone, Debug, PartialEq)]
pub struct Need {
    pub path: String,
    pub always: bool,
    pub only_if: Option<String>,
}

/// Tests that all hold, as Python: `a and b is None`.
fn only_if_text(tests: &intern::IdSet<intern::StrId>) -> String {
    let par = |t: &str| if t.contains(" or ") { format!("({})", t) } else { t.to_string() };
    tests.iter().map(|t| par(&intern::str_get(t))).collect::<Vec<_>>().join(" and ")
}

/// One step of an access path: `.name` for an attribute, `["k"]` / `[0]` / `[*]` for an item.
fn step_text(k: KeyId, attr: bool) -> String {
    if k == STAR {
        "[*]".into()
    } else if attr {
        format!(".{}", intern::key_py_str(k))
    } else {
        format!("[{}]", crate::domain::key_display(k))
    }
}

/// What functions return, when it is a dict or a frame with known keys: one line per
/// function, joined over every call (`make_user returns {"id": …, "role"?: …}`).
fn result_lines(an: &Analyzer) -> Vec<String> {
    // (function of the program, a frame?) -> the rows of what it returned, joined
    let program: std::collections::HashSet<String> = (0..an.heap.len() as ObjId).filter_map(|o| an.heap.func_info_opt(o)).filter(|fi| an.is_local_module(fi.module)).map(|fi| fi.qualname.clone()).collect();
    let mut by_func: IndexMap<(String, bool), Row> = IndexMap::new();
    for ((qual, _), (_, rows)) in an.findings.summaries.iter().filter(|((q, _), _)| program.contains(q)) {
        for (oid, row) in rows.iter().filter(|(o, r)| matches!(an.heap.kind(*o), Kind::Dict | Kind::Frame) && !r.present.is_empty()) {
            let key = (qual.trim_start_matches("class ").to_string(), an.heap.kind(*oid) == Kind::Frame);
            let joined = by_func.get(&key).map(|r| r.join(row)).unwrap_or_else(|| row.clone());
            by_func.insert(key, joined);
        }
    }
    by_func.sort_keys();
    let mut out: IndexMap<String, Vec<String>> = IndexMap::new();
    for ((f, frame), row) in by_func {
        let mut items: Vec<String> = intern::sorted_by_str(row.present.iter()).iter().filter(|k| !is_internal_key(**k)).map(|k| format!("{}{}{}", crate::domain::key_display(*k), row.mark(*k), if frame { "" } else { ": …" })).collect();
        if !row.closed() {
            items.push("**?".into());
        }
        out.entry(f).or_default().push(if frame { format!("DataFrame[{}]", items.join(", ")) } else { format!("{{{}}}", items.join(", ")) });
    }
    out.into_iter().map(|(f, parts)| format!("  {} returns {}", f, parts.join(" | "))).collect()
}

/// The analysis' own limits, condensed: calls into code rowan cannot see, functions it
/// stopped following, deadlines, and the other notes.
fn limit_lines(an: &Analyzer) -> Vec<String> {
    let mut unknown: Vec<String> = Vec::new();
    let (mut n_unknown, mut n_skipped) = (0, 0);
    let mut budget: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for n in an.findings.notes() {
        if let Some(site) = n.strip_prefix("unknown call ") {
            // (calls of the program's own functions rowan stopped following: counted with them)
            if an.names.skipped_sites.contains(&crate::intern::s(site)) {
                n_skipped += 1;
                continue;
            }
            n_unknown += 1;
            let q = site.rsplit_once("@L").map(|(q, _)| q).unwrap_or(site);
            // (a library base class's method reached through `super()`)
            let q = q.strip_prefix("super.").map_or_else(|| q.to_string(), |m| format!("super().{}", m));
            if named_call(site) && !unknown.contains(&q) {
                unknown.push(q);
            }
        } else if let Some(q) = n.strip_suffix(" ran out of its budget: later calls are unknown code") {
            budget.push(format!("`{}`", q.trim_start_matches("class ")));
        } else if n.starts_with("analysis cut at the deadline") {
            out.push(format!("  {}", n.replace("analysis cut at the deadline", "the analysis reached its time limit").replace("unknown code", "treated as code rowan cannot see")));
        } else {
            out.push(format!("  {}", n));
        }
    }
    let mut lines = Vec::new();
    if n_unknown > 0 {
        let eg = if unknown.is_empty() { String::new() } else { format!(", e.g. {}", unknown.iter().take(6).map(|u| format!("`{}`", u)).collect::<Vec<_>>().join(", ")) };
        lines.push(format!("  {} call{} into code rowan cannot see (it may return anything and change what it is given){}", n_unknown, if n_unknown == 1 { " goes" } else { "s go" }, eg));
    }
    if !budget.is_empty() {
        let n = budget.len();
        let shown: Vec<String> = budget.into_iter().take(6).collect();
        let later = if n_skipped > 0 { format!("{} ", n_skipped) } else { String::new() };
        lines.push(format!("  rowan stopped following {} function{} after many calls, so their {}later calls count as code it cannot see: {}{}", n, if n == 1 { "" } else { "s" }, later, shown.join(", "), if n > 6 { ", …" } else { "" }));
    }
    lines.extend(out);
    lines
}

/// What each mark the report uses means (only the marks it uses).
fn legend(text: &str) -> Vec<String> {
    let has_mark = |sym: &str| text.lines().any(|l| l.split_whitespace().nth(1) == Some(sym));
    let mut out = Vec::new();
    let mut add = |used: bool, mark: &str, meaning: &str| {
        if used {
            out.push(format!("  {:<12} {}", mark, meaning));
        }
    };
    add(has_mark("✗"), "✗", "fails on every path that reaches it");
    add(has_mark("⚠"), "⚠", "fails on some paths");
    add(has_mark("⚠~"), "⚠~", "fails unless code rowan cannot see sets the key first");
    add(text.lines().any(|l| l.contains(" ×") && (l.contains(" ✗ ") || l.contains(" ⚠"))), "×2", "written that many times on the line, each with this finding");
    add(text.contains("  via L"), "via L5→L9", "the calls (by line) through which the code was reached");
    add(text.contains("  always:"), "always", "read on every path through the code");
    add(text.contains("  sometimes:"), "sometimes", "read on some paths only");
    add(text.contains("  only if "), "only if x:", "read only on calls where x holds of the arguments as passed");
    add(text.contains("[*]"), "[*]", "any element");
    add(text.lines().any(|l| (l.contains("  always:") || l.contains("  sometimes:")) && l.contains("..")), "[0..6]", "each index from 0 to 6");
    let values = ["\n== what functions return", "\n== module-level variables"].iter().find_map(|h| text.find(h)).map(|i| &text[i..]).unwrap_or("");
    let values = values.split("\n== limits").next().unwrap_or("");
    let plain = values.replace("**?", "").replace("\"?", "").replace("?=", "=").replace("?:", ":");
    add(plain.contains('?'), "?", "a value rowan cannot know (it depends on input or on code rowan cannot see)");
    add(values.contains("?:") || values.contains("?=") || values.contains("\"?,") || values.contains("\"?]"), "\"k\"?: k?=", "a key / attribute / column present on some paths only");
    add(values.contains("~:") || values.contains("~=") || values.contains("\"~,") || values.contains("\"~]"), "\"k\"~: k~=", "present unless code rowan cannot see removed it");
    add(values.contains("**?"), "**?", "other keys / attributes / columns may exist");
    add(values.contains(" | "), "a | b", "one of these, depending on the path");
    add(values.contains('…'), "…", "not shown");
    out
}

pub fn verdict_json(an: &Analyzer, v: &Verdict) -> J {
    json!({
        "kind": "verdict",
        "site": format!("{}:{}:{}", if v.file.is_empty() { an.program.filename.clone() } else { v.file.clone() }, v.line, v.col),
        "file": v.file,
        "line": v.line,
        "col": v.col,
        "key": key_repr(v.key),
        "key_str": intern::key_py_str(v.key),
        "ns": v.ns.as_str(),
        "tier": v.tier.name(),
        "guarded": v.guarded,
        "object": v.obj,
        "chain": v.chain,
        "note": v.note,
        "providers": v.providers.iter().map(|p| source_label(*p)).collect::<Vec<_>>(),
        "text": v.text,
    })
}

pub fn to_json(an: &Analyzer) -> Vec<J> {
    let mut out: Vec<J> = Vec::new();
    for v in reported(an) {
        out.push(verdict_json(an, &v));
    }
    for (label, entry) in schema_rows(an) {
        let keys: serde_json::Map<String, J> = entry.into_iter().collect();
        out.push(json!({"kind": "schema", "source": label, "keys": keys}));
    }
    out
}

/// The verdicts that are diagnostics: not those on accesses the program guards.
fn reported(an: &Analyzer) -> Vec<Verdict> {
    dedupe(an, an.findings.verdicts()).into_iter().filter(|v| !v.guarded).collect()
}

/// Everything the golden harness needs, as one JSON document (guarded verdicts included).
pub fn golden_json(an: &Analyzer) -> J {
    let verdicts: Vec<J> = dedupe(an, an.findings.verdicts()).iter().map(|v| verdict_json(an, v)).collect();
    let mut schemas = serde_json::Map::new();
    for (label, entry) in schema_rows(an) {
        let keys: serde_json::Map<String, J> = entry.into_iter().collect();
        schemas.insert(label, J::Object(keys));
    }
    let rows: Vec<J> = an
        .findings
        .row_checks
        .iter()
        .map(|rc| {
            let value = json!({"lits": rc.value.lits.iter().map(|k| key_repr(*k)).collect::<Vec<_>>(), "unknown": rc.value.unknown, "objects": rc.value.objects, "types": rc.value.types, "untyped": rc.value.untyped});
            let mut out = match &rc.row {
            None => json!({"file": rc.file, "line": rc.line, "name": rc.name, "row": J::Null, "value": value}),
            Some(r) => json!({
                "file": rc.file, "line": rc.line, "name": rc.name, "value": value,
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
            if let J::Object(o) = &mut out {
                o.insert("general".into(), json!(rc.general));
                o.insert("quiet".into(), json!(rc.quiet));
            }
            if let (Some(J::Object(t)), J::Object(o)) = (&rc.tensor, &mut out) {
                for (k, v) in t {
                    o.insert(k.clone(), v.clone());
                }
            }
            out
        })
        .collect();
    let note = |n: &crate::models::torch::ShapeNote| json!({"file": an.program.rel_file(n.module), "line": n.line, "chain": n.chain, "entry": n.entry, "text": n.text, "message": n.text});
    let requires: Vec<J> = an.m.shape_reqs.iter().map(note).collect();
    let shape_errors: Vec<J> = an.m.shape_errors.iter().map(note).collect();
    json!({"verdicts": verdicts, "schemas": schemas, "rows": rows, "notes": an.findings.notes(), "report": format_text(an, true, true), "requires": requires, "shape_errors": shape_errors, "objects": an.heap.len(), "tabulations": an.budget.runs, "joins": crate::domain::row_joins(), "work": an.budget.statements, "visits": an.budget.walked.get() + crate::state::fp_visits(), "havoc": an.havoc.visited})
}


/// A value as the user would write it, as far as it is known: literals (`1`,
/// `"a"`, several as `1 | 2 | ?`), dicts with their keys and values (`{"a": 1, "b"?: ?, **?}`),
/// lists / tuples by position when known (`[1, 2]`, `(1,)`), else `list[X]` /
/// `tuple[X, ...]`, instances as `Cls(a=1, **?)`, functions / classes by name, `?` for
/// anything unknown (`**?`: other keys or attributes may exist).  Depth and width are
/// limited (`…` marks what is not shown; also a cycle).
/// How deep rendered values go before nested containers are cut to `[…]` / `{…}` / `Name(…)`
/// (`--value-depth N`; default 3).
pub static VALUE_DEPTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(3);

/// Values rendered at several runs of a line as one: their alternatives (split at the top
/// level, outside brackets and strings), each once, the unknown `?` last.
pub fn join_alternatives(texts: &[String]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for t in texts {
        let (mut depth, mut quote, mut start) = (0i32, None::<char>, 0usize);
        let mut prev = ' ';
        for (i, c) in t.char_indices() {
            match (quote, c) {
                (Some(q), _) if c == q && prev != '\\' => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(c),
                (None, '(' | '[' | '{') => depth += 1,
                (None, ')' | ']' | '}') => depth -= 1,
                (None, '|') if depth == 0 && t[..i].ends_with(' ') && t[i + 1..].starts_with(' ') => {
                    parts.push(t[start..i - 1].trim());
                    start = i + 2;
                }
                _ => {}
            }
            prev = c;
        }
        parts.push(t[start..].trim());
    }
    let mut out: Vec<&str> = Vec::new();
    for p in parts {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    if let Some(i) = out.iter().position(|p| *p == "?") {
        let q = out.remove(i);
        out.push(q);
    }
    out.join(" | ")
}

pub fn render_value(an: &Analyzer, state: &State, v: &Value, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let mut parts: Vec<String> = Vec::new();
    // literals
    let lits = crate::domain::kv_literals(&v.kv);
    if v.kv.is_literal() && !lits.is_empty() {
        let mut ls: Vec<String> = intern::sorted_by_str(lits.iter()).iter().map(|k| short_literal(crate::domain::key_display(*k))).collect();
        if ls.len() > 4 {
            ls.truncate(4);
            ls.push("…".into());
        }
        parts.extend(ls);
    }
    // a tensor parameter with known shape facts: its shape
    if let Some(s) = crate::models::torch::t_known_param_shape(an, v, state) {
        return crate::models::torch::tensor_text(&s);
    }
    for o in v.pts.iter() {
        parts.push(render_object(an, state, o, depth, seen));
    }
    // a part nothing is known about (data, keys of something): `?`
    if let Some(t) = crate::models::torch::t_render(an, v, state) {
        parts.push(t);
    } else if let Some(expr) = data_expr(&v.kv) {
        parts.push(expr);
    } else if let Some(t) = v.kv.data_site().and_then(|l| crate::domain::label_type(&intern::str_get(l))) {
        parts.push(format!("{}(?)", t));
    } else if !v.kv.is_literal() && !v.kv.is_bot() {
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
    if !an.heap.has_obj(o) {
        return "?".into();
    }
    let kind = an.heap.kind(o);
    if seen.contains(&o) {
        // a cycle back to an object being printed
        return "…".into();
    }
    let deeper = depth + 1;
    match kind {
        Kind::Dict | Kind::Frame => render_mapping(an, state, o, kind, depth, seen),
        // objects of library models: their attributes when they have some (`Namespace(n=3)`),
        // else what made them (`re.compile('a+')`, `PosixPath('/tmp')`) or their type (`<Match>`)
        Kind::Namespace => {
            let label = an.heap.obj(o).label.to_string();
            let name = opaque_name(&label);
            let row = state.row(o, Ns::Attr);
            // (attribute bags: argparse's and types' namespaces, a request's `state`; other model
            // objects keep their attributes for the model's own use)
            let bag = ["args@", "namespace@", "request.state"].iter().any(|p| label.starts_with(p));
            if bag && row.present.iter().any(|k| !is_internal_key(k)) {
                let shown = name.trim_matches(|c| c == '<' || c == '>');
                if depth >= max_depth {
                    return format!("{}(…)", shown);
                }
                return render_record(an, state, o, Ns::Attr, Some(shown), depth, seen);
            }
            name
        }
        // a (key, value) pair of `items()` / `zip` / `enumerate`: a 2-tuple
        Kind::Pair => {
            let crate::interp::Payload::Pair(k, v) = &an.heap.obj(o).payload else { return "?".into() };
            if depth >= max_depth {
                return "(…)".into();
            }
            seen.push(o);
            let out = format!("({}, {})", render_value(an, state, k, deeper, seen), render_value(an, state, v, deeper, seen));
            seen.pop();
            out
        }
        Kind::List | Kind::Tuple | Kind::Set => render_sequence(an, state, o, kind, depth, seen),
        Kind::Instance => {
            let name = an.heap.instance_class(o).and_then(|c| an.heap.class_info_opt(c)).map(|ci| ci.name.clone()).unwrap_or_else(|| "object".into());
            if depth >= max_depth {
                return format!("{}(…)", name);
            }
            render_record(an, state, o, Ns::Attr, Some(&name), depth, seen)
        }
        Kind::Function => {
            let l = an.heap.obj(o).label.trim_start_matches("def ").to_string();
            match l.strip_prefix("lambda@L") {
                Some(line) => format!("<lambda on line {}>", line),
                None => l,
            }
        }
        Kind::Bound => {
            let l = an.heap.obj(o).label.clone();
            let name = l.trim_start_matches("def ").trim_start_matches("class ").split(" bound").next().unwrap_or(&l).to_string();
            format!("<bound method {}>", name)
        }
        Kind::Class => an.heap.obj(o).label.trim_start_matches("class ").to_string(),
        Kind::Module => an.heap.obj(o).label.to_string(),
        // an unknown named in the program's terms (`a`, `a(b)`, `cfg["db"]`), else `?`
        // (unnamed here: of its type when it has one, `int(?)`)
        Kind::Unknown => an.names.visible(o).cloned().unwrap_or_else(|| match an.heap.obj(o).ty {
            Some(t) => format!("{}(?)", t),
            None => "?".into(),
        }),
        Kind::External => an.heap.external_qual(o).trim_start_matches("builtins.").to_string(),
        Kind::Super => "super()".into(),
        // iterators, views, groupbys, parsers, responses…: by their type, as Python prints them
        _ => opaque_name(&an.heap.obj(o).label),
    }
}

/// A list, tuple or set: its known positions in order (a set's known literal members sorted),
/// else what any element may be in typing notation (`list[int]`); a deque wrapped.
fn render_sequence(an: &Analyzer, state: &State, o: ObjId, kind: Kind, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let max_depth: u32 = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    const MAX_ITEMS: usize = 8;
    let deeper = depth + 1;
    let (open, close, ty) = match kind {
        Kind::List => ("[", "]", "list"),
        Kind::Tuple => ("(", ")", "tuple"),
        _ => ("{", "}", "set"),
    };
    seen.push(o);
    let out = match an.heap.llen(o, state) {
        // a generator: by its function (it runs when it is consumed)
        _ if an.heap.obj(o).label.starts_with("generator ") => format!("<{}>", an.heap.obj(o).label),
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
        // a set of known literal members: them, sorted (a set has no order)
        Some(n) if kind == Kind::Set && depth < max_depth && n <= MAX_ITEMS && set_members(state, o, n).is_some() => {
            let ms = set_members(state, o, n).unwrap();
            if ms.is_empty() {
                "set()".into()
            } else {
                format!("{{{}}}", ms.iter().map(|k| render_value(an, state, &Value::lit(*k), deeper, seen)).collect::<Vec<_>>().join(", "))
            }
        }
        // one of a few lengths (an append on some paths): each list (`["a"] | ["a", "b"]`)
        None if kind != Kind::Set && depth < max_depth && alt_lists(an, state, o).is_some() => {
            let alts = alt_lists(an, state, o).unwrap();
            let items: Vec<String> = (0..*alts.last().unwrap()).map(|i| render_value(an, state, &state.slot(o, Ns::Pos, intern::key_int(i as i64)), deeper, seen)).collect();
            alts.iter().map(|n| if kind == Kind::Tuple && *n == 1 { format!("({},)", items[0]) } else { format!("{}{}{}", open, items[..*n].join(", "), close) }).collect::<Vec<_>>().join(" | ")
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
    if an.heap.obj(o).label.starts_with("deque@") {
        return format!("deque({})", out);
    }
    out
}

/// The lengths a list may have, when they are 2 or 3 short ones whose positions are known.
fn alt_lists(an: &Analyzer, state: &State, o: ObjId) -> Option<Vec<usize>> {
    let alts = an.heap.len_alts(o, state)?;
    let max = *alts.last()?;
    ((2..=3).contains(&alts.len()) && max <= 4 && (0..max).all(|i| state.has_slot(o, Ns::Pos, intern::key_int(i as i64)))).then_some(alts)
}

/// A dict or frame: its keys and values (a frame by its columns; a Series / Counter /
/// defaultdict / OrderedDict wrapped as Python prints them).
fn render_mapping(an: &Analyzer, state: &State, o: ObjId, kind: Kind, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let max_depth: u32 = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    let row = state.row(o, Ns::Item);
    // (a frame by its columns, a Series / Counter / defaultdict by their keys, as
    // Python prints them)
    let label = an.heap.obj(o).label.clone();
    let wrap = if kind == Kind::Frame { Some("DataFrame") } else { ["series@", "Counter@", "defaultdict@", "OrderedDict@"].iter().find(|p| label.starts_with(**p)).map(|p| p.trim_end_matches('@')) };
    let body = if depth >= max_depth {
        if row.present.is_empty() && row.closed() { "{}".into() } else { "{…}".into() }
    } else if kind == Kind::Frame {
        let mut cols: Vec<String> = intern::sorted_by_str(row.present.iter()).iter().map(|k| format!("{}{}", crate::domain::key_display(*k), row.mark(*k))).collect();
        if !row.closed() {
            cols.push("**?".into());
        }
        return format!("DataFrame[{}]", cols.join(", "));
    } else {
        render_record(an, state, o, Ns::Item, None, depth, seen)
    };
    match wrap {
        // (nothing known about its keys: the type alone)
        Some(w) if body == "{**?}" => format!("<{}>", if w == "series" { "Series" } else { w }),
        Some("series") => format!("Series({})", body),
        Some(w) => format!("{}({})", w, body),
        None => body,
    }
}

/// A long string literal cut to its first 60 characters (`"abc…"`).
fn short_literal(t: String) -> String {
    const MAX: usize = 60;
    if t.chars().count() <= MAX || !t.starts_with('"') {
        return t;
    }
    format!("{}…\"", t.chars().take(MAX - 2).collect::<String>())
}

/// An object of a library model by its type (`<Match>`, `<sqlite3.Connection>`), or by the
/// call that made it when its label reads as one (`re.compile('a+')`, `PosixPath('/tmp')`).
fn opaque_name(label: &str) -> String {
    if label.contains('(') && !label.contains('@') {
        return label.to_string();
    }
    let base = label.split('@').next().unwrap_or(label).trim();
    let base = base.replace("stdobj.repattern.", "re.Pattern.").replace("stdobj.", "");
    let name = match base.as_str() {
        "sqlite3.connect" => "sqlite3.Connection",
        "fastapi.app" => "FastAPI",
        "fastapi.client" => "TestClient",
        "fastapi.response" | "response" => "Response",
        "items view" => "dict_items",
        "keys view" => "dict_keys",
        "values view" => "dict_values",
        "groupby" => "GroupBy",
        "args" => "Namespace",
        "request.state" => "State",
        "" => "object",
        b => b,
    };
    format!("<{}>", name)
}

/// Keys the models keep for themselves (`"\u{1}text"`): never shown.
fn is_internal_key(k: KeyId) -> bool {
    intern::key_as_str(k).map(|s| s.starts_with('\u{1}')).unwrap_or(false)
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
    let rest = crate::domain::label_name(&label).strip_prefix("expr:")?;
    let (scope, text) = rest.split_once(':')?;
    let here = RENDER_SCOPE.with(|c| c.get());
    (scope.parse::<u32>().ok()? == here).then(|| text.to_string())
}

/// The keys (`ns` Item: `{"a": 1, "b"?: ?, **?}`) or attributes (Attr: `Name(a=1, **?)`) of an
/// object, with their values.
fn render_record(an: &Analyzer, state: &State, o: ObjId, ns: Ns, name: Option<&str>, depth: u32, seen: &mut Vec<ObjId>) -> String {
    const MAX_ITEMS: usize = 8;
    let row = state.row(o, ns);
    seen.push(o);
    let mut items: Vec<String> = Vec::new();
    let keys: Vec<KeyId> = intern::sorted_by_str(row.present.iter()).into_iter().filter(|k| !is_internal_key(*k)).collect();
    for k in keys.iter().take(MAX_ITEMS) {
        let val = match state.slot_opt(o, ns, *k) {
            Some(sv) => render_value(an, state, &sv, depth + 1, seen),
            None => "?".into(),
        };
        if ns == Ns::Attr {
            items.push(format!("{}{}={}", intern::key_py_str(*k), row.mark(*k), val));
        } else {
            // (an object used as a key: the object, by name)
            let key = match an.heap.keys.identity_objs.borrow().get(k).copied() {
                Some(ko) => render_object(an, state, ko, VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed), seen),
                None => crate::domain::key_display(*k),
            };
            items.push(format!("{}{}: {}", key, row.mark(*k), val));
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

/// The symbols a value mentions (unknown objects printed by name) with what is known about
/// each: the keys / attributes seen on it, what an unknown call may return, else `?`.
pub fn value_legend(an: &Analyzer, state: &State, v: &Value) -> Vec<(String, String)> {
    fn walk(an: &Analyzer, state: &State, v: &Value, depth: u32, seen: &mut Vec<ObjId>, out: &mut Vec<(String, String)>) {
        if depth > VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed) || out.len() >= 8 {
            return;
        }
        for o in v.pts.iter() {
            if seen.contains(&o) {
                continue;
            }
            seen.push(o);
            match an.heap.kind(o) {
                Kind::Unknown => {
                    let Some(sym) = an.names.visible(o) else { continue };
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
                    if let Some((_, _, given)) = an.names.origins.get(&o) {
                        parts.extend(given.iter().cloned());
                    }
                    // keys seen on it describe it; objects an unknown call was given are what it
                    // may also be (besides anything else)
                    let given = an.names.origins.get(&o).map(|(_, _, g)| !g.is_empty()).unwrap_or(false);
                    let desc = if parts.is_empty() { "?".to_string() } else if given { format!("{} | ?", parts.join(" | ")) } else { parts.join(" | ") };
                    out.push((sym.clone(), desc));
                }
                // (what a container holds: under its keys, attributes, positions, elements)
                Kind::Dict | Kind::Frame | Kind::Namespace | Kind::Instance | Kind::List | Kind::Tuple | Kind::Set => {
                    for ns in [Ns::Item, Ns::Attr, Ns::Pos, Ns::Elem] {
                        for (_, sv) in state.obj_slots(o, ns) {
                            walk(an, state, &sv, depth + 1, seen, out);
                        }
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

/// The `--facts` document for the editor: per (line, name) the value as text, the names it
/// mentions, the objects it may be (for people: where they were made and changed), what the
/// code reads of it, and the findings in words.
pub fn facts_json(an: &Analyzer) -> J {
    let mut facts: Vec<J> = Vec::new();
    for ((line, name), f) in &an.findings.facts {
        // (a hover lists the objects a name may hold: past `MAX_FACT_OBJECTS` only the count —
        // a name holding hundreds of objects made the document hundreds of MB)
        let objects_total = f.shown_rows().len();
        let objects: Vec<J> = f
            .shown_rows()
            .iter()
            .take(MAX_FACT_OBJECTS)
            .map(|(oid, (item, attr))| fact_object_json(an, f, *oid, item, attr))
            .collect();
        // (the unknowns of the value for any call, when the objects shown are the calls': what
        // the hover says of where the value's unknowns come from)
        let general_unknowns: Option<Vec<J>> = (!f.rows.is_empty() && !f.general.is_empty()).then(|| {
            f.general_rows.iter().filter(|(o, _)| an.heap.kind(**o) == Kind::Unknown).take(MAX_FACT_OBJECTS).map(|(oid, (item, attr))| fact_object_json(an, f, *oid, item, attr)).collect()
        });
        // what the code reads of the inputs these objects come from, as access paths from the name
        let required = fact_required(an, f, *name);
        let required: Vec<J> = required
            .into_iter()
            .map(|n| match n.only_if {
                Some(w) => json!({"path": n.path, "always": n.always, "only_if": w}),
                None => json!({"path": n.path, "always": n.always}),
            })
            .collect();
        // tensor-shape requirements recorded on this line (in the line's own analysis), in
        // the dim names of this fact
        let (requires, extra_legend) = fact_shape_requires(an, f, *line);
        // the value for any call (the general run's), and apart from it what the program's
        // calls gave (with their lines); without a general run (a closure, a lambda) what the
        // calls gave is the value
        let (text, calls): (String, Vec<J>) = if f.general.is_empty() {
            (join_alternatives(&f.calls.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>()), vec![])
        } else {
            let g = join_alternatives(&f.general);
            let calls = f.calls.iter().filter(|(t, _)| *t != g).map(|(t, lines)| json!({"text": t, "lines": lines.iter().filter(|l| **l > 0).collect::<Vec<_>>()})).collect();
            (g, calls)
        };
        facts.push(json!({"line": line, "name": name.to_string(), "text": text, "general": !f.general.is_empty(), "calls": calls, "legend": f.legend.iter().filter(|(_, d)| d != "?").chain(extra_legend.iter()).map(|(s0, d)| json!([s0, d])).collect::<Vec<_>>(), "objects": objects, "objects_total": objects_total, "general_unknowns": general_unknowns, "required": required, "requires": requires}));
    }
    // (columns in UTF-16 units, as editors count them; each finding with its message in words)
    let main_idx = an.heap.module_info(an.program.main_module).idx;
    let verdicts: Vec<J> = reported(an)
        .iter()
        .map(|v| {
            let mut j = verdict_json(an, v);
            if v.file.is_empty() {
                j["col"] = json!(crate::frontend::utf16_col(main_idx, v.line, v.col));
            }
            j["message"] = json!(verdict_message(an, v));
            j
        })
        .collect();
    let shape_errors: Vec<J> = an.m.shape_errors.iter().map(|n| json!({"file": an.program.rel_file(n.module), "line": n.line, "chain": n.chain, "message": n.text})).collect();
    json!({"file": an.program.filename, "facts": facts, "verdicts": verdicts, "shape_errors": shape_errors})
}

/// One object a hover's name may hold: what may have changed it, where it comes from, what it
/// never has, its label, kind, origin and symbol.
fn fact_object_json(an: &Analyzer, f: &crate::interp::Fact, oid: ObjId, item: &Option<Row>, attr: &Option<Row>) -> J {
    let o = an.heap.obj(oid);
    let origin = an.names.origins.get(&oid).map(|(t, l, g)| json!({"text": t, "line": l, "given": g}));
    // for people: what may have changed it, what it never has, where it comes from
    let rows = [item, attr].into_iter().flatten();
    let mut changed_by: Vec<String> = Vec::new();
    let mut from: Vec<String> = Vec::new();
    let mut absent: Vec<String> = Vec::new();
    for r in rows {
        for m in r.mutators.iter() {
            let t = site_text(an, &intern::str_get(m));
            if !changed_by.contains(&t) {
                changed_by.push(t);
            }
            changed_by.sort_by_key(|n| n == "other calls");
        }
        for (src, _) in r.sources.iter() {
            let t = source_id_text(an, src);
            if !from.contains(&t) {
                from.push(t);
            }
        }
        absent.extend(intern::sorted_by_str(r.absent.iter()).iter().map(|k| crate::domain::key_display(*k)));
    }
    json!({
        "changed_by": changed_by,
        "from": from,
        "absent": absent,
        "label": &*o.label,
        "kind": o.kind.name(),
        "origin": origin,
        "symbol": an.names.get(oid).filter(|_| f.legend.iter().any(|(s0, _)| Some(s0) == an.names.get(oid))),
    })
}

/// What the code reads of the inputs a hover's objects come from, as access paths from the
/// name (with whether every path reads it).
fn fact_required(an: &Analyzer, f: &crate::interp::Fact, name: crate::intern::StrId) -> Vec<Need> {
    let mut required: Vec<Need> = Vec::new();
    if let Some(st) = &an.findings.final_state {
        for (_, (item, attr)) in f.shown_rows() {
            for r in [item, attr].into_iter().flatten() {
                for (src, _) in r.sources.iter() {
                    if source_is_data(src) {
                        continue;
                    }
                    for need in reads_below(st, src, &intern::str_get(name), false) {
                        if !required.iter().any(|n| n.path == need.path) {
                            required.push(need);
                        }
                    }
                }
            }
        }
    }
    required
}

/// The tensor-shape requirements recorded on a hover's line (in the line's own analysis), in
/// the fact's dim names, with the legend entries for the names they use.
#[allow(clippy::type_complexity)]
fn fact_shape_requires(an: &Analyzer, f: &crate::interp::Fact, line: u32) -> (Vec<String>, Vec<(String, String)>) {
    let mut requires: Vec<String> = Vec::new();
    let mut extra_legend: Vec<(String, String)> = Vec::new();
    for n in &an.m.shape_reqs {
        if n.line != line || !n.chain.is_empty() || n.module != an.program.main_idx {
            continue;
        }
        let t = match &n.cons {
            Some((c, ctx)) => {
                let mut ctx = ctx.clone();
                ctx.atom_names = f.t_names.clone();
                crate::models::shape::with_ctx(ctx, || crate::models::shape::cons_text(c))
            }
            None => n.text.clone(),
        };
        // the dim names the requirement uses, in the legend too
        if let Some((c, ctx)) = &n.cons {
            let mut atoms = Vec::new();
            cons_atoms(c, &mut atoms);
            for a in atoms {
                if let (Some(nm), crate::models::shape::Atom::Dim(sy, i)) = (f.t_names.get(&a), crate::models::shape::atom_get(a)) {
                    let base = crate::models::shape::with_ctx(ctx.clone(), || crate::models::shape::sym_print(sy));
                    if let Some(b) = base {
                        let entry = (nm.clone(), format!("{}.shape[{}]", b, i));
                        if !extra_legend.contains(&entry) && !f.legend.iter().any(|(s0, _)| s0.split(", ").any(|x| x == nm)) {
                            extra_legend.push(entry);
                        }
                    }
                }
            }
        }
        if !requires.contains(&t) {
            requires.push(t);
        }
    }
    (requires, extra_legend)
}

fn cons_atoms(c: &crate::models::shape::Cons, out: &mut Vec<crate::models::shape::AtomId>) {
    match c {
        crate::models::shape::Cons::Eq(a, b) => {
            crate::models::shape::dim_atoms(*a, out);
            crate::models::shape::dim_atoms(*b, out);
        }
        crate::models::shape::Cons::Rank(..) => {}
        crate::models::shape::Cons::If(m, g) => {
            cons_atoms(m, out);
            g.iter().for_each(|x| cons_atoms(x, out));
        }
    }
}

/// The members of set `o` of known length `n` when every position holds one literal, sorted.
fn set_members(state: &State, o: ObjId, n: usize) -> Option<Vec<crate::intern::KeyId>> {
    let mut ms = Vec::new();
    for i in 0..n {
        let sv = state.slot_opt(o, Ns::Pos, intern::key_int(i as i64))?;
        ms.push(sv.kv.as_lit().filter(|_| sv.pts.is_empty())?);
    }
    ms.sort_by(|a, b| crate::interp::compare::literal_order(*a, *b).unwrap_or_else(|| intern::key_repr(*a).cmp(&intern::key_repr(*b))));
    ms.dedup();
    Some(ms)
}

#[cfg(test)]
mod tests {
    #[test]
    fn index_runs() {
        assert_eq!(super::index_runs(&["t[1]", "t[0]", "t[2]", "x", "t[4]", "u[1]", "u[2]"]), ["t[0..2]", "t[4]", "x", "u[1]", "u[2]"]);
    }

    #[test]
    fn join_alternatives_dedupes_top_level() {
        let t = |xs: &[&str]| super::join_alternatives(&xs.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(t(&["{\"a\": 1} | None", "None | {\"a\": 1}"]), "{\"a\": 1} | None");
        assert_eq!(t(&["{\"a\": 1 | 2}", "?", "3"]), "{\"a\": 1 | 2} | 3 | ?");
        assert_eq!(t(&["\"x | y\" | ?", "\"x | y\""]), "\"x | y\" | ?");
    }
}
