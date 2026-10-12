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
        let names: IndexMap<u32, String> = by_obj.iter().map(|(o, v)| (*o, v.obj.clone())).collect();
        let mut tiers: Vec<Tier> = by_obj.values().map(|v| v.tier).collect();
        tiers.sort();
        tiers.dedup();
        if tiers.contains(&Tier::Unknown) {
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
        } else {
            Tier::Possibly
        };
        let mut provs: Vec<String> = by_obj.values().flat_map(|v| v.providers.clone()).collect();
        provs.sort();
        provs.dedup();
        out.push(Verdict {
            line: first.line,
            col: first.col,
            key: first.key,
            tier,
            obj: object_list(names.values()),
            chain: first.chain.clone(),
            note: format!("depends on which object: {}", breakdown.join("; ")),
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
pub fn schema_rows(an: &Analyzer) -> IndexMap<String, IndexMap<String, bool>> {
    let mut out: IndexMap<String, IndexMap<String, bool>> = IndexMap::new();
    let Some(st) = &an.final_state else { return out };
    for (src, d) in st.reqs.items().into_iter().filter(|(s, _)| !source_is_data(*s)) {
        let entry = out.entry(source_label(src)).or_default();
        for (k, info) in d.iter() {
            entry.insert(key_repr(*k), info.always);
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
    let mut lines: Vec<String> = Vec::new();
    let all = reported(an);
    lines.push("== accesses that may fail ==".into());
    let problems = problem_lines(an, &all);
    if problems.is_empty() {
        lines.push("  (none found)".into());
    }
    lines.extend(problems);
    let oks = all.iter().filter(|v| v.tier == Tier::Proven).count();
    let likely = all.iter().filter(|v| v.tier == Tier::Likely).count();
    if oks + likely > 0 {
        let mut ok_line = format!("  ({}{} access{} always succeed{}", oks, if lines.len() > 2 { " other" } else { "" }, if oks == 1 { "" } else { "es" }, if oks == 1 { "s" } else { "" });
        if likely > 0 {
            ok_line += &format!("; {} more unless code rowan cannot see removed the key", likely);
        }
        lines.push(ok_line + ")");
    }
    // tensor shapes: definite mismatches, then what each line needs of its inputs
    if an.torch_on {
        lines.push(String::new());
        lines.push("== tensor shapes ==".into());
        let mut seen: Vec<(u32, String)> = Vec::new();
        let mut errs: Vec<&crate::models::torch::ShapeNote> = an.m.shape_errors.iter().collect();
        errs.sort_by_key(|e| (e.line, e.chain.len()));
        for e in errs {
            if seen.contains(&(e.line, e.text.clone())) {
                continue;
            }
            seen.push((e.line, e.text.clone()));
            lines.push(format!("  L{:<5} ✗  shape mismatch: {}{}", e.line, e.text, via_text(&e.chain, 0)));
        }
        let mut reqs: Vec<(u32, String)> = an.m.shape_reqs.iter().filter(|r| r.chain.is_empty()).map(|r| (r.line, r.text.clone())).collect();
        reqs.sort();
        reqs.dedup();
        for (line, text) in &reqs {
            lines.push(format!("  L{:<5}    needs {}", line, text));
        }
        if seen.is_empty() && reqs.is_empty() {
            lines.push("  (no mismatches, no requirements)".into());
        }
    }
    let (inputs, contracts) = needs_text(an);
    for (title, group) in [("== what the code reads from its inputs ==", inputs), ("== what each function needs from its arguments ==", contracts)] {
        if group.is_empty() {
            continue;
        }
        lines.push(String::new());
        lines.push(title.into());
        lines.extend(group);
    }
    if show_summaries {
        let results = result_lines(an);
        if !results.is_empty() {
            lines.push(String::new());
            lines.push("== what functions return ==".into());
            lines.extend(results);
        }
    }
    if show_rows {
        if let Some(st) = &an.final_state {
            let m = an.main_module;
            let mut names: Vec<(String, crate::domain::Value)> = st.obj_slots(m, Ns::Attr).into_iter().map(|(k, v)| (intern::key_py_str(k), v)).collect();
            names.sort_by(|a, b| a.0.cmp(&b.0));
            // definitions (functions, classes, imported modules) are not values worth listing
            let vals: Vec<String> = names
                .into_iter()
                .filter(|(_, v)| v.pts.is_empty() || !v.pts.iter().all(|o| matches!(an.kind(o), Kind::Function | Kind::Class | Kind::Module | Kind::External)))
                .map(|(name, v)| {
                    // (one line each, cut at 150 characters: the hover has the rest)
                    let line = format!("  {} = {}", name, render_value(an, st, &v, 0, &mut Vec::new()));
                    if line.chars().count() > 150 { format!("{}…", line.chars().take(149).collect::<String>()) } else { line }
                })
                .collect();
            if !vals.is_empty() {
                lines.push(String::new());
                lines.push("== module-level variables at the end ==".into());
                lines.extend(vals);
            }
        }
    }
    let limits = limit_lines(an);
    if !limits.is_empty() {
        lines.push(String::new());
        lines.push("== limits of this analysis ==".into());
        lines.extend(limits);
    }
    let body = lines.join("\n");
    let legend = legend(&body);
    if legend.is_empty() {
        return body;
    }
    format!("{}\n\n== legend ==\n{}", body, legend.join("\n"))
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
    let width = groups.keys().map(|k| k.3.chars().count()).max().unwrap_or(0).min(36);
    // (grouped by file: the main file first, then the others under their names)
    groups.sort_by(|a, _, b, _| (!a.0.is_empty(), &a.0, a.1, a.2).cmp(&(!b.0.is_empty(), &b.0, b.1, b.2)));
    let mut out = Vec::new();
    let mut file_now = String::new();
    for ((file, line, _, text, mm), vs) in groups {
        if file != file_now {
            out.push(format!("  {}:", file));
            file_now = file;
        }
        let (mark, msg) = mm.split_once('\u{0}').unwrap();
        // (reached directly: no call path to show)
        let via = if vs.iter().any(|v| v.chain.is_empty()) { String::new() } else { via_text(&vs[0].chain, vs.len() - 1) };
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
        _ if an.has_obj(v.oid) && an.kind(v.oid) == Kind::Frame => format!("column {}", crate::domain::key_display(v.key)),
        _ => format!("key {}", crate::domain::key_display(v.key)),
    };
    let unknown_code = || code_list(an, &v.mutators);
    let (note, other_kind) = match v.note.strip_suffix("; the value may also be another kind of value") {
        Some(n) => (n, true),
        None => (v.note.as_str(), false),
    };
    let msg = match (v.tier, note) {
        (Tier::Proven, _) => format!("{} is always there", what),
        (Tier::Likely, _) => format!("{} is there, unless {} removed it", what, unknown_code()),
        (Tier::Input, _) => {
            let from: Vec<String> = v.providers.iter().map(|p| source_text(an, p)).collect();
            format!("{} must be provided by {}", what, from.join(" or "))
        }
        (Tier::LikelyMissing, _) => format!("{} is never set, unless {} added it", what, unknown_code()),
        (Tier::DataKey, _) => "the key is computed from data, and may not be one of the keys this object has".to_string(),
        (_, "set on some paths only") => format!("{} is only set on some paths", what),
        (_, "bound on some paths only") => format!("{} is only assigned on some paths", what),
        (_, "bound on no path to here") => format!("{} is not assigned on any path to here", what),
        (_, "all keys known") | (_, "no source can provide it") => format!("{} is never set", what),
        (_, "removed/excluded on every path") => format!("{} was removed or renamed on every path to here", what),
        (_, "key from another record's key set") => format!("{}: the key comes from another object's keys", what),
        (_, n) if n.starts_with("depends on which object: ") => format!("{} is not on every object this may be ({})", what, &n["depends on which object: ".len()..]),
        (_, "") => format!("{} may be missing", what),
        (_, n) => format!("{}: {}", what, n),
    };
    if other_kind {
        format!("{}, unless the value is a plain one such as a str", msg)
    } else {
        msg
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
        let f = an.rel_file(m);
        if f.is_empty() { format!("line {}", line) } else { format!("{} line {}", f, line) }
    };
    if let Some((text, m, line)) = an.site_names.get(&intern::s(site)) {
        let t: String = if text.chars().count() > 50 { format!("{}…", text.chars().take(49).collect::<String>()) } else { text.clone() };
        return format!("`{}` ({})", t, in_file(*m, *line));
    }
    if site == "…" {
        // (a row names its first few unknown calls, then this)
        return "other calls".into();
    }
    let clean = |q: &str| q.trim_start_matches("class ").trim_start_matches("builtins.").to_string();
    if let Some(q) = site.strip_prefix("recursion@") {
        return format!("the recursion in `{}`", clean(q));
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

/// A source label (`json.loads@L6['db']`) for people: its site and the path below it.
fn source_text(an: &Analyzer, label: &str) -> String {
    let (site, path) = match label.find("['").or_else(|| label.find("[-")).or_else(|| label.find("[0")) {
        Some(i) if !label[..i].ends_with("?") => (&label[..i], &label[i..]),
        _ => (label, ""),
    };
    if path.is_empty() {
        return site_text(an, site);
    }
    // (the path inside the code span: `json.loads(…)["db"]`, argument `items[*]` of `f`)
    let path = path.replace("['*']", "[*]").replace("['", "[\"").replace("']", "\"]");
    let t = site_text(an, site);
    match t.find("` ").or_else(|| t.ends_with('`').then(|| t.len() - 1)) {
        Some(i) if t[..i].contains('`') => format!("{}{}{}", &t[..i], path, &t[i..]),
        _ => format!("{} {}", t, path),
    }
}

/// An input for people, by id: its site and the path below it, each step written as the
/// code read it (`argument `items[*].sku` of `f``).
fn source_id_text(an: &Analyzer, src: SourceId) -> String {
    let (site, kind, path) = intern::with_source(src, |s| (s.site.clone(), s.kind, s.path.clone()));
    let t = site_text(an, &site);
    let Some(st) = an.final_state.as_ref().filter(|_| !path.is_empty()) else { return t };
    let steps = path_steps(st, &site, kind, 0, &path).map(|(p, _)| p).unwrap_or_else(|| "…".into());
    match t.find("` ").or_else(|| t.ends_with('`').then(|| t.len() - 1)) {
        Some(i) if t[..i].contains('`') => format!("{}{}{}", &t[..i], steps, &t[i..]),
        _ => format!("{} {}", t, steps),
    }
}

/// The access path of `path[from..]` below an input (`.sku`, `["db"]`, `[*]`), each step as
/// the parent's requirement says it was read; whether a step is an attribute; None for a
/// path through what a method returned (`df.groupby(…)["amount"]`: a step the code called,
/// with no requirement of its own).
fn path_steps(st: &State, site: &std::sync::Arc<str>, kind: crate::domain::SourceKind, from: usize, path: &[KeyId]) -> Option<(String, bool)> {
    let mut out = String::new();
    let mut attr_step = false;
    for i in from..path.len() {
        let parent = intern::source(crate::domain::Source { site: site.clone(), kind, path: path[..i].to_vec() });
        let attr = match st.reqs.get(&parent, &path[i]) {
            Some(r) => r.attr,
            None if path[i] != STAR && intern::key_as_str(path[i]).is_some() => return None,
            None => false,
        };
        attr_step |= attr;
        out += &step_text(path[i], attr);
    }
    Some((out, attr_step))
}

/// Is an unknown call's result an input worth listing (a library function the program
/// calls: `requests.get`, `read_csv`), not a value rowan lost track of (`?`, `unknown.strip`,
/// a call of a call's result, a function past its budget)?
fn named_call(site: &str) -> bool {
    let q = site.rsplit_once("@L").map(|(q, _)| q).unwrap_or(site);
    !q.is_empty() && !q.starts_with("unknown.") && !q.starts_with("builtins.") && q.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

/// What the code needs of its inputs and of each function's arguments, as Python access
/// paths: (inputs, functions).
fn needs_text(an: &Analyzer) -> (Vec<String>, Vec<String>) {
    use crate::domain::SourceKind;
    let Some(st) = &an.final_state else { return (vec![], vec![]) };
    // (functions of the program that rowan did not follow are not inputs)
    let program: std::collections::HashSet<String> = (0..an.objects.len() as ObjId).filter_map(|o| an.func_info_opt(o)).map(|fi| fi.py_qualname()).collect();
    // (an input made in another module: its file, from the reads that needed it)
    let mut files: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for v in an.verdicts.iter().filter(|v| !v.file.is_empty()) {
        for p in &v.providers {
            files.entry(p.split('[').next().unwrap_or(p).to_string()).or_insert_with(|| v.file.clone());
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
    let mut groups: IndexMap<(bool, String), Vec<(String, bool)>> = IndexMap::new();
    for (param, header, root, base) in roots {
        // (attributes of what a library call returned are its API, not data the code needs)
        groups.entry((param, header)).or_default().extend(reads_below(st, root, &base, !param));
    }
    let (mut inputs, mut funcs) = (Vec::new(), Vec::new());
    let width = groups.keys().map(|(_, h)| h.chars().count()).max().unwrap_or(0).min(30);
    for ((param, header), needs) in groups {
        let always: Vec<&str> = needs.iter().filter(|(_, a)| *a).map(|(p, _)| p.as_str()).collect();
        let sometimes: Vec<&str> = needs.iter().filter(|(_, a)| !*a).map(|(p, _)| p.as_str()).collect();
        if needs.is_empty() {
            continue;
        }
        let out = if param { &mut funcs } else { &mut inputs };
        let mut head = format!("  {:<w$}", header, w = width);
        if header.chars().count() > width {
            out.push(format!("  {}", header));
            head = format!("  {:<w$}", "", w = width);
        }
        let pad = format!("  {:<w$}", "", w = width);
        for (i, (label, ps)) in [("always", &always), ("sometimes", &sometimes)].into_iter().filter(|(_, ps)| !ps.is_empty()).enumerate() {
            out.push(format!("{}  {:<10} {}", if i == 0 { &head } else { &pad }, format!("{}:", label), ps.join(", ")));
        }
    }
    (inputs, funcs)
}

/// What the code reads of input `src` and of what lies below it, as access paths from
/// `base` (`row["id"]`, `items[*].sku`): (path, read on every path), the longest paths only.
/// `data_only`: keys and elements, not attributes.
pub fn reads_below(st: &State, src: SourceId, base: &str, data_only: bool) -> Vec<(String, bool)> {
    let (site, kind, top) = intern::with_source(src, |s| (s.site.clone(), s.kind, s.path.clone()));
    let mut needs: Vec<(String, bool)> = Vec::new();
    for (s2, d) in st.reqs.items() {
        let (site2, kind2, path) = intern::with_source(s2, |s| (s.site.clone(), s.kind, s.path.clone()));
        if site2 != site || kind2 != kind || !path.starts_with(&top) {
            continue;
        }
        let Some((prefix, attr_step)) = path_steps(st, &site, kind, top.len(), &path) else { continue };
        if data_only && attr_step {
            continue;
        }
        for (k, info) in d.iter() {
            if data_only && info.attr {
                continue;
            }
            // (a key unknown code or another input may supply is needed only sometimes)
            let always = info.always && info.caveats.is_empty() && info.alternatives.is_empty();
            let mut text = format!("{}{}{}", base, prefix, step_text(*k, info.attr));
            for a in info.alt_keys.iter().filter(|a| a != k) {
                text += &format!(" or {}{}{}", base, prefix, step_text(a, info.attr));
            }
            needs.push((text, always));
        }
    }
    needs.sort();
    needs.dedup_by(|a, b| a.0 == b.0 && {
        b.1 = b.1 && a.1;
        true
    });
    // (a path read below another needs that one too: only the longer is listed)
    let all: Vec<String> = needs.iter().map(|(p, _)| p.clone()).collect();
    needs.retain(|(p, _)| !all.iter().any(|q| q.len() > p.len() && q.starts_with(p.as_str()) && matches!(q.as_bytes()[p.len()], b'[' | b'.')));
    needs
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
    let mut by_func: IndexMap<String, IndexMap<String, Row>> = IndexMap::new();
    for ((qual, _), (_, rows)) in &an.summaries {
        for (oid, row) in rows {
            let kind = an.kind(*oid);
            if !matches!(kind, Kind::Dict | Kind::Frame) || row.present.is_empty() {
                continue;
            }
            let shape = if kind == Kind::Frame { "frame" } else { "dict" };
            let e = by_func.entry(qual.trim_start_matches("class ").to_string()).or_default();
            match e.get_mut(shape) {
                Some(r) => *r = r.join(row),
                None => {
                    e.insert(shape.to_string(), row.clone());
                }
            }
        }
    }
    by_func.sort_keys();
    by_func
        .into_iter()
        .map(|(f, shapes)| {
            let parts: Vec<String> = shapes
                .iter()
                .map(|(shape, row)| {
                    let mut items: Vec<String> = intern::sorted_by_str(row.present.iter()).iter().filter(|k| !is_internal_key(**k)).map(|k| format!("{}{}", crate::domain::key_display(*k), row_mark(row, *k))).collect();
                    if !row.closed() {
                        items.push("**?".into());
                    }
                    if shape == "frame" {
                        format!("DataFrame[{}]", items.join(", "))
                    } else {
                        format!("{{{}}}", items.iter().map(|i| if i == "**?" { i.clone() } else { i.replacen("\"?", "\"?: …", 1).replacen("\"~", "\"~: …", 1) }).map(|i| if i.ends_with('"') { format!("{}: …", i) } else { i }).collect::<Vec<_>>().join(", "))
                    }
                })
                .collect();
            format!("  {} returns {}", f, parts.join(" | "))
        })
        .collect()
}

/// The analysis' own limits, condensed: calls into code rowan cannot see, functions it
/// stopped following, deadlines, and the other notes.
fn limit_lines(an: &Analyzer) -> Vec<String> {
    let mut unknown: Vec<String> = Vec::new();
    let mut n_unknown = 0;
    let mut budget: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for n in &an.notes {
        if let Some(site) = n.strip_prefix("unknown call ") {
            n_unknown += 1;
            let q = site.rsplit_once("@L").map(|(q, _)| q).unwrap_or(site);
            if named_call(site) && !unknown.iter().any(|u| u == q) {
                unknown.push(q.to_string());
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
        let mut l = format!("  {} call{} into code rowan cannot see (it may return anything and change what it is given)", n_unknown, if n_unknown == 1 { " goes" } else { "s go" });
        if !unknown.is_empty() {
            let shown: Vec<String> = unknown.iter().take(6).map(|u| format!("`{}`", u)).collect();
            l += &format!(", e.g. {}", shown.join(", "));
        }
        lines.push(l);
    }
    if !budget.is_empty() {
        let n = budget.len();
        let shown: Vec<String> = budget.into_iter().take(6).collect();
        lines.push(format!("  rowan stopped following {} function{} after many calls, so their later calls count as code it cannot see: {}{}", n, if n == 1 { "" } else { "s" }, shown.join(", "), if n > 6 { ", …" } else { "" }));
    }
    lines.extend(out);
    lines
}

/// What each mark the report uses means (only the marks it uses).
fn legend(text: &str) -> Vec<String> {
    let diag = text.split("\n== ").next().unwrap_or("");
    let has_mark = |sym: &str| diag.lines().any(|l| l.split_whitespace().nth(1) == Some(sym));
    let mut out = Vec::new();
    let mut add = |used: bool, mark: &str, meaning: &str| {
        if used {
            out.push(format!("  {:<12} {}", mark, meaning));
        }
    };
    add(has_mark("✗"), "✗", "fails on every path that reaches it");
    add(has_mark("⚠"), "⚠", "fails on some paths");
    add(has_mark("⚠~"), "⚠~", "fails unless code rowan cannot see sets the key first");
    add(diag.contains("  via L") || text.contains("✗  shape mismatch") && text.contains("  via L"), "via L5→L9", "the calls (by line) through which the code was reached");
    add(text.contains("  always:"), "always", "read on every path through the code");
    add(text.contains("  sometimes:"), "sometimes", "read on some paths only");
    add(text.contains("[*]"), "[*]", "any element");
    let values = ["\n== what functions return", "\n== module-level variables"].iter().find_map(|h| text.find(h)).map(|i| &text[i..]).unwrap_or("");
    let values = values.split("\n== limits").next().unwrap_or("");
    let plain = values.replace("**?", "").replace("\"?", "").replace("?=", "=");
    add(plain.contains('?'), "?", "a value rowan cannot know (it depends on input or on code rowan cannot see)");
    add(values.contains("\"?:") || values.contains("?=") || values.contains("\"?,") || values.contains("\"?]"), "\"k\"?: k?=", "a key / attribute / column present on some paths only");
    add(values.contains("~:") || values.contains("~=") || values.contains("\"~,") || values.contains("\"~]"), "\"k\"~: k~=", "present unless code rowan cannot see removed it");
    add(values.contains("**?"), "**?", "other keys / attributes / columns may exist");
    add(values.contains(" | "), "a | b", "one of these, depending on the path");
    add(values.contains('…'), "…", "not shown");
    out
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
        "guarded": v.guarded,
        "object": v.obj,
        "chain": v.chain,
        "note": v.note,
        "providers": v.providers,
        "text": v.text,
    })
}

pub fn to_json(an: &Analyzer) -> Vec<J> {
    let mut out: Vec<J> = Vec::new();
    for v in reported(an) {
        out.push(verdict_json(an, &v));
    }
    for (label, entry) in schema_rows(an) {
        let keys: serde_json::Map<String, J> = entry.iter().map(|(k, a)| (k.clone(), json!({"always": a}))).collect();
        out.push(json!({"kind": "schema", "source": label, "keys": keys}));
    }
    out
}

/// The verdicts that are diagnostics: not those on accesses the program guards.
fn reported(an: &Analyzer) -> Vec<Verdict> {
    dedupe(an, &an.verdicts).into_iter().filter(|v| !v.guarded).collect()
}

/// Everything the golden harness needs, as one JSON document (guarded verdicts included).
pub fn golden_json(an: &Analyzer) -> J {
    let verdicts: Vec<J> = dedupe(an, &an.verdicts).iter().map(|v| verdict_json(an, v)).collect();
    let mut schemas = serde_json::Map::new();
    for (label, entry) in schema_rows(an) {
        let keys: serde_json::Map<String, J> = entry.iter().map(|(k, a)| (k.clone(), json!({"always": a}))).collect();
        schemas.insert(label, J::Object(keys));
    }
    let rows: Vec<J> = an
        .row_checks
        .iter()
        .map(|rc| {
            let value = json!({"lits": rc.value.lits.iter().map(|k| key_repr(*k)).collect::<Vec<_>>(), "unknown": rc.value.unknown, "objects": rc.value.objects});
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
            if let (Some(J::Object(t)), J::Object(o)) = (&rc.tensor, &mut out) {
                for (k, v) in t {
                    o.insert(k.clone(), v.clone());
                }
            }
            out
        })
        .collect();
    let note = |n: &crate::models::torch::ShapeNote| json!({"line": n.line, "chain": n.chain, "entry": n.entry, "text": n.text, "message": n.text});
    let requires: Vec<J> = an.m.shape_reqs.iter().map(note).collect();
    let shape_errors: Vec<J> = an.m.shape_errors.iter().map(note).collect();
    json!({"verdicts": verdicts, "schemas": schemas, "rows": rows, "notes": an.notes, "report": format_text(an, true, true), "requires": requires, "shape_errors": shape_errors, "objects": an.objects.len(), "tabulations": an.tab_runs, "joins": crate::domain::row_joins(), "work": an.work, "visits": an.walked.get() + crate::state::fp_visits()})
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
    if let Some(s) = an.t_known_param_shape(v, state) {
        return crate::models::torch::tensor_text(&s);
    }
    for o in v.pts.iter() {
        parts.push(render_object(an, state, o, depth, seen));
    }
    // a part nothing is known about (data, keys of something): `?`
    if let Some(t) = an.t_render(v, state) {
        parts.push(t);
    } else if let Some(expr) = data_expr(&v.kv) {
        parts.push(expr);
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
        Kind::Dict | Kind::Frame => {
            let row = state.row(o, Ns::Item);
            // (a frame by its columns, a Series / Counter / defaultdict by their keys, as
            // Python prints them)
            let label = an.obj(o).label.clone();
            let wrap = if kind == Kind::Frame { Some("DataFrame") } else { ["series@", "Counter@", "defaultdict@", "OrderedDict@"].iter().find(|p| label.starts_with(**p)).map(|p| p.trim_end_matches('@')) };
            let body = if depth >= max_depth {
                if row.present.is_empty() && row.closed() { "{}".into() } else { "{…}".into() }
            } else if kind == Kind::Frame {
                let mut cols: Vec<String> = intern::sorted_by_str(row.present.iter()).iter().map(|k| format!("{}{}", crate::domain::key_display(*k), row_mark(&row, *k))).collect();
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
        // objects of library models: their attributes when they have some (`Namespace(n=3)`),
        // else what made them (`re.compile('a+')`, `PosixPath('/tmp')`) or their type (`<Match>`)
        Kind::Namespace => {
            let label = an.obj(o).label.to_string();
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
            let crate::interp::Payload::Pair(k, v) = &an.obj(o).payload else { return "?".into() };
            if depth >= max_depth {
                return "(…)".into();
            }
            seen.push(o);
            let out = format!("({}, {})", render_value(an, state, k, deeper, seen), render_value(an, state, v, deeper, seen));
            seen.pop();
            out
        }
        Kind::List | Kind::Tuple | Kind::Set => {
            let (open, close, ty) = match kind {
                Kind::List => ("[", "]", "list"),
                Kind::Tuple => ("(", ")", "tuple"),
                _ => ("{", "}", "set"),
            };
            seen.push(o);
            let out = match an.llen(o, state) {
                // a generator: by its function (it runs when it is consumed)
                _ if an.obj(o).label.starts_with("generator ") => format!("<{}>", an.obj(o).label),
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
            if an.obj(o).label.starts_with("deque@") {
                return format!("deque({})", out);
            }
            out
        }
        Kind::Instance => {
            let name = an.instance_class(o).and_then(|c| an.class_info_opt(c)).map(|ci| ci.name.clone()).unwrap_or_else(|| "object".into());
            if depth >= max_depth {
                return format!("{}(…)", name);
            }
            render_record(an, state, o, Ns::Attr, Some(&name), depth, seen)
        }
        Kind::Function => {
            let l = an.obj(o).label.trim_start_matches("def ").to_string();
            match l.strip_prefix("lambda@L") {
                Some(line) => format!("<lambda on line {}>", line),
                None => l,
            }
        }
        Kind::Bound => {
            let l = an.obj(o).label.clone();
            let name = l.trim_start_matches("def ").split(" bound").next().unwrap_or(&l).to_string();
            format!("{} (bound)", name)
        }
        Kind::Class => an.obj(o).label.trim_start_matches("class ").to_string(),
        Kind::Module => an.obj(o).label.to_string(),
        // an unknown named in the program's terms (`a`, `a(b)`, `cfg["db"]`), else `?`
        Kind::Unknown => an.symbol_for(o).cloned().unwrap_or_else(|| "?".into()),
        Kind::External => an.external_qual(o).trim_start_matches("builtins.").to_string(),
        Kind::Super => "super()".into(),
        // iterators, views, groupbys, parsers, responses…: by their type, as Python prints them
        _ => opaque_name(&an.obj(o).label),
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
    let name = match base {
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

/// How a key is marked in a rendered record: present on every path (""), on some paths
/// only ("?"), there unless unknown code removed it ("~").
fn row_mark(r: &Row, k: KeyId) -> &'static str {
    if r.definite.contains(k) {
        ""
    } else if r.likely.contains(k) {
        "~"
    } else {
        "?"
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
    let mark = row_mark;
    seen.push(o);
    let mut items: Vec<String> = Vec::new();
    let keys: Vec<KeyId> = intern::sorted_by_str(row.present.iter()).into_iter().filter(|k| !is_internal_key(*k)).collect();
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
                Kind::Unknown => {
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

/// The `--facts` document for the editor: per (line, name) the value as text, the names it
/// mentions, the objects it may be (for people: where they were made and changed), what the
/// code reads of it, and the findings in words.
pub fn facts_json(an: &Analyzer) -> J {
    let mut facts: Vec<J> = Vec::new();
    for ((line, name), f) in &an.facts {
        // (a hover lists the objects a name may hold: past `MAX_FACT_OBJECTS` only the count —
        // a name holding hundreds of objects made the document hundreds of MB)
        let objects_total = f.rows.len();
        let objects: Vec<J> = f
            .rows
            .iter()
            .take(MAX_FACT_OBJECTS)
            .map(|(oid, (item, attr))| {
                let o = an.obj(*oid);
                let origin = an.origins.get(oid).map(|(t, l, g)| json!({"text": t, "line": l, "given": g}));
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
                    "symbol": an.symbols.get(oid).filter(|_| f.legend.iter().any(|(s0, _)| Some(s0) == an.symbols.get(oid))),
                })
            })
            .collect();
        // what the code reads of the inputs these objects come from, as access paths from the name
        let mut required: Vec<(String, bool)> = Vec::new();
        if let Some(st) = &an.final_state {
            for (_, (item, attr)) in &f.rows {
                for r in [item, attr].into_iter().flatten() {
                    for (src, _) in r.sources.iter() {
                        if source_is_data(src) {
                            continue;
                        }
                        for need in reads_below(st, src, &intern::str_get(*name), false) {
                            if !required.iter().any(|(p, _)| *p == need.0) {
                                required.push(need);
                            }
                        }
                    }
                }
            }
        }
        let required: Vec<J> = required.into_iter().map(|(p, a)| json!({"path": p, "always": a})).collect();
        // tensor-shape requirements recorded on this line (in the line's own analysis), in
        // the dim names of this fact
        let mut requires: Vec<String> = Vec::new();
        let mut extra_legend: Vec<(String, String)> = Vec::new();
        for n in &an.m.shape_reqs {
            if n.line != *line || !n.chain.is_empty() {
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
        let (text, examples): (String, Vec<String>) = match &f.primary {
            Some(p) => (p.clone(), f.texts.iter().filter(|t| *t != p).cloned().collect()),
            None => (f.texts.join(" | "), vec![]),
        };
        facts.push(json!({"line": line, "name": name.to_string(), "text": text, "examples": examples, "legend": f.legend.iter().filter(|(_, d)| d != "?").chain(extra_legend.iter()).map(|(s0, d)| json!([s0, d])).collect::<Vec<_>>(), "objects": objects, "objects_total": objects_total, "required": required, "requires": requires}));
    }
    // (columns in UTF-16 units, as editors count them; each finding with its message in words)
    let main_idx = an.module_info(an.main_module).idx;
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
    let shape_errors: Vec<J> = an.m.shape_errors.iter().map(|n| json!({"line": n.line, "chain": n.chain, "message": n.text})).collect();
    json!({"file": an.filename, "facts": facts, "verdicts": verdicts, "shape_errors": shape_errors})
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
    ms.sort_by(|a, b| crate::interp::exprs::literal_order(*a, *b).unwrap_or_else(|| intern::key_repr(*a).cmp(&intern::key_repr(*b))));
    ms.dedup();
    Some(ms)
}
