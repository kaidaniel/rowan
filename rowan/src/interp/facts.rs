//! The `--facts` document the editor's hovers and diagnostics are made from (`rowan serve`
//! sends it, `vscode-rowan` renders it): per (line, name) the value as text (for any call, and
//! what each of the program's calls gave), the names it mentions, the objects it may be (where
//! they were made, what changed them, where their unknowns come from), what the code reads of
//! it (access paths; a parameter's contract), tensor-shape requirements, and the findings in
//! words.

use serde_json::{json, Value as J};

use crate::domain::{source_is_data, Ns, ObjId, Row, Tier};
use crate::intern;
use crate::interp::report::{reads_below, reported, site_text, source_id_text, verdict_json, verdict_message, Need, MAX_FACT_OBJECTS};
use crate::interp::render::{is_internal_key, join_alternatives};
use crate::interp::Analyzer;
use crate::state::Kind;

/// The `--facts` document for the editor: per (line, name) the value as text, the names it
/// mentions, the objects it may be (for people: where they were made and changed), what the
/// code reads of it, and the findings in words.
pub fn facts_json(an: &Analyzer) -> J {
    let mut facts: Vec<J> = Vec::new();
    for ((line, name), f) in &an.findings.facts {
        // (the objects a name may hold, for where they were made, where its unknowns come from
        // and what changed them: the first `MAX_FACT_OBJECTS` — a name holding hundreds of
        // objects made the document hundreds of MB — and how many there are)
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
        // the dim names of this fact; a parameter's at its `def`: its function's for any call
        // that name it
        let (requires, extra_legend) = match f.param_of.and_then(|p| an.heap.func_info_opt(p)) {
            Some(fi) => {
                let (a, b) = (fi.node.node().line, fi.node.node().end_line);
                let word = |t: &str| t.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == &*name.to_string());
                fact_shape_requires(an, f, &|n| n.general && a <= n.line && n.line <= b, &|t| word(t))
            }
            None => fact_shape_requires(an, f, &|n| n.line == *line, &|_| true),
        };
        // the value for any call (the general run's), and apart from it what the program's
        // calls gave (with their lines); without a general run (a closure, a lambda) what the
        // calls gave is the value
        let call = |t: &String, lines: &Vec<u32>| {
            let mut c = json!({"text": t, "lines": lines.iter().filter(|l| **l > 0).collect::<Vec<_>>()});
            if f.param_of.is_some() {
                c["check"] = call_check(an, f, lines);
            }
            c
        };
        let (text, calls): (String, Vec<J>) = if f.general.is_empty() {
            (join_alternatives(&f.calls.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>()), vec![])
        } else {
            let g = join_alternatives(&f.general);
            let calls = f.calls.iter().filter(|(t, _)| *t != g || f.param_of.is_some()).map(|(t, lines)| call(t, lines)).collect();
            (g, calls)
        };
        let mut fact = json!({"line": line, "name": name.to_string(), "text": text, "general": !f.general.is_empty(), "calls": calls, "legend": f.legend.iter().filter(|(_, d)| d != "?").chain(extra_legend.iter()).map(|(s0, d)| json!([s0, d])).collect::<Vec<_>>(), "objects": objects, "objects_total": objects_total, "general_unknowns": general_unknowns, "required": required, "requires": requires});
        // a parameter at its `def`: what the function reads of it for any call (its contract)
        if let Some(foid) = f.param_of {
            fact["param"] = json!(true);
            fact["contract"] = param_contract(an, foid, *name);
        }
        // (an unknown the name stands for, of the builtin type its annotation gives it: `x: int`)
        let own = |o: &ObjId| an.heap.kind(*o) == Kind::Unknown && an.names.get(*o).is_some_and(|n| *n == name.to_string());
        // (a builtin container's: with its parts' types, `list[dict[str(?), int(?)]]`)
        if let Some(o) = f.general_rows.keys().chain(f.rows.keys()).filter(|o| own(o)).find(|o| an.heap.obj(**o).ty.is_some()) {
            fact["type"] = match an.heap.ann.get(o) {
                Some(a) => json!(crate::interp::entry::annotation_text(a)),
                None => json!(an.heap.obj(*o).ty),
            };
        }
        facts.push(fact);
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
    json!({"file": an.program.filename, "facts": facts, "verdicts": verdicts, "shape_errors": shape_errors, "analysis": an.analysis_json()})
}

/// One object a hover's name may hold: what may have changed it, where it comes from, its kind,
/// origin and symbol.
fn fact_object_json(an: &Analyzer, f: &crate::interp::Fact, oid: ObjId, item: &Option<Row>, attr: &Option<Row>) -> J {
    let origin = an.names.origins.get(&oid).map(|(t, l, g)| json!({"text": t, "line": l, "given": g}));
    // for people: what may have changed it, where it comes from
    let rows = [item, attr].into_iter().flatten();
    let mut changed_by: Vec<String> = Vec::new();
    let mut from: Vec<String> = Vec::new();
    // (keys and attributes it never has here)
    let absent = |r: &Option<Row>| r.as_ref().map(|r| intern::sorted_by_str(r.absent.iter()).iter().filter(|k| !is_internal_key(**k)).map(|k| crate::domain::key_display(*k)).collect::<Vec<_>>()).unwrap_or_default();
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
    }
    // where the program made it: its file (empty: the hovered one) and line
    let site = an.heap.obj(oid).site.clone();
    let (module, rest) = match site.strip_prefix('M').and_then(|r| r.split_once(':')) {
        Some((m, r)) => (m.parse::<u32>().ok(), r),
        None => (Some(an.program.main_idx), &*site),
    };
    let line = rest.strip_prefix('L').and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|l| l.parse::<u32>().ok());
    // (of a module with a file: not a model's own, `re`'s patterns)
    let created = match (module, line) {
        (Some(m), Some(l)) if an.heap.kind(oid) != Kind::Unknown && (m == an.program.main_idx || crate::frontend::module_path(m).is_some_and(|p| p.ends_with(".py"))) => json!({"file": an.program.rel_file(m), "line": l}),
        _ => J::Null,
    };
    json!({
        "changed_by": changed_by,
        "from": from,
        "created": created,
        "absent": absent(item),
        "absent_attrs": absent(attr),
        "kind": an.heap.kind(oid).name(),
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

/// What function `foid`'s runs for any call read of its parameter `name`: access paths, on
/// every path (`always`) or under tests of its parameters (`only_if`).
fn param_contract(an: &Analyzer, foid: ObjId, name: crate::intern::StrId) -> J {
    let (Some(st), Some(fi)) = (&an.findings.contracts, an.heap.func_info_opt(foid)) else { return json!([]) };
    let src = crate::domain::Source::new(&format!("param {}.{}", fi.py_qualname(), name), crate::domain::SourceKind::Param);
    let needs = reads_below(st, src, &crate::intern::str_get(name), false);
    J::Array(needs.into_iter().map(|n| json!({"path": n.path, "always": n.always, "only_if": n.only_if})).collect())
}

/// Whether what the program's calls on `lines` passed for a parameter has what the function
/// reads of it: the worst of the accesses of those calls' runs (in the function, its callees
/// too) on what they passed, with the accesses that may fail (`mark`: ✓ ✓~ ◌ ⚠~ ⚠ ✗).
fn call_check(an: &Analyzer, f: &crate::interp::Fact, lines: &[u32]) -> J {
    let passed: Vec<&(crate::domain::Chain, Vec<ObjId>)> = f.passed.iter().filter(|(c, _)| c.last().is_some_and(|l| lines.contains(l))).collect();
    let Some(st) = &an.findings.final_state else { return J::Null };
    if passed.is_empty() {
        return J::Null;
    }
    // (what they passed and what it holds)
    let mut reach: rustc_hash::FxHashSet<ObjId> = rustc_hash::FxHashSet::default();
    let mut work: Vec<ObjId> = passed.iter().flat_map(|(_, o)| o.iter().copied()).collect();
    while let Some(o) = work.pop() {
        if reach.len() > 512 || !reach.insert(o) || matches!(an.heap.kind(o), Kind::Module | Kind::Class | Kind::Function) {
            continue;
        }
        for ns in [Ns::Item, Ns::Attr, Ns::Pos, Ns::Elem] {
            for (_, v) in st.obj_slots(o, ns) {
                work.extend(v.pts.iter());
            }
        }
    }
    // (a chain into the function: the access's chain goes through it)
    let within = |vc: &[u32], c: &[u32]| (0..vc.len()).any(|p| vc[..=p].len() <= c.len() && c.ends_with(&vc[..=p]) && vc[p] == *c.last().unwrap());
    const ORDER: [Tier; 6] = [Tier::Missing, Tier::Possibly, Tier::LikelyMissing, Tier::Input, Tier::Likely, Tier::Proven];
    let mut worst: Option<usize> = None;
    let mut fails: Vec<String> = Vec::new();
    for v in an.findings.verdicts() {
        if !reach.contains(&v.oid) || !passed.iter().any(|(c, _)| !c.is_empty() && within(&v.chain, c)) {
            continue;
        }
        let Some(i) = ORDER.iter().position(|t| *t == v.tier) else { continue };
        worst = Some(worst.map_or(i, |w| w.min(i)));
        if i <= 2 && !fails.contains(&v.text) {
            fails.push(v.text.clone());
        }
    }
    // (tensor shapes: a shape error in those calls' runs)
    let mut errors: Vec<String> = Vec::new();
    for n in &an.m.shape_errors {
        if n.module == an.program.main_idx && passed.iter().any(|(c, _)| !c.is_empty() && within(&n.chain, c)) && !errors.contains(&n.text) {
            errors.push(n.text.clone());
            worst = Some(0);
        }
    }
    let Some(w) = worst else { return J::Null };
    json!({"tier": ORDER[w].name(), "fails": fails, "errors": errors})
}

/// The tensor-shape requirements a hover shows (`pick`: those of its line in the line's own
/// analysis, or a parameter's function's that name it), in the fact's dim names, with the
/// legend entries for the names they use.
#[allow(clippy::type_complexity)]
fn fact_shape_requires(an: &Analyzer, f: &crate::interp::Fact, pick: &dyn Fn(&crate::models::torch::ShapeNote) -> bool, names: &dyn Fn(&str) -> bool) -> (Vec<String>, Vec<(String, String)>) {
    let mut requires: Vec<String> = Vec::new();
    let mut extra_legend: Vec<(String, String)> = Vec::new();
    for n in &an.m.shape_reqs {
        if !pick(n) || !n.chain.is_empty() || n.module != an.program.main_idx {
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
        if !names(&t) {
            continue;
        }
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

/// The dim atoms a shape constraint mentions.
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
