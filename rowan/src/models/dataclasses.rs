//! `dataclasses` (and the classes built like them: NamedTuple, TypedDict): the generated
//! `__init__` (fields in MRO order, defaults, `field(...)`, `InitVar`, `KW_ONLY`,
//! `__post_init__`); abc: a class with abstract methods left unimplemented cannot be made.

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, Classification, Ns, ObjId, Tier, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::interp::{Analyzer, Frame, FrameKind};
use crate::state::{Kind, State};

/// A dataclass field as `__init__` takes it (`dc_specs`).
#[derive(Clone, Debug)]
pub struct DcSpec {
    pub name: String,
    pub default: Option<&'static ast::Expr>,
    pub init: bool,
    pub initvar: bool,
    pub kw_only: bool,
}

/// The generated `__init__` of a dataclass / TypedDict / NamedTuple `cls` run on `inst`
/// (a fresh instance, or `self` when called explicitly as `Base.__init__(self, …)` /
/// `super().__init__(…)` from a subclass).
pub fn dataclass_init(an: &mut Analyzer, cls: u32, inst: ObjId, args: &[Value], star_args: &[Value], kwargs: &IndexMap<StrId, Value>, star_kwargs: &[Value], state: &mut State, node: &Node) {
    let ci = an.heap.class_info(cls);
    let ns = if ci.lib.typed_dict { Ns::Item } else { Ns::Attr };
    let mut initvars: Vec<Value> = Vec::new();
    // `*seq` arguments: a sequence of known length gives positional arguments in order; past
    // one of unknown length every further positional field may come from it (its element)
    let mut args_full: Vec<Value> = args.to_vec();
    let mut star_rest: Option<Value> = None;
    for sa in star_args {
        if star_rest.is_some() {
            break;
        }
        // (a list / tuple of known length: its positions)
        if let Some(o) = sa.single().filter(|o| matches!(an.heap.kind(*o), Kind::Tuple | Kind::List) && sa.kv.is_bot()) {
            if let Some(n) = an.heap.llen(o, state) {
                for i in 0..n {
                    args_full.push(state.slot(o, Ns::Pos, intern::key_int(i as i64)));
                }
                continue;
            }
        }
        let it = an.iterate(sa, node, state);
        match it.items {
            Some(items) if it.exhaustive => args_full.extend(items),
            _ => {
                star_rest = Some(match it.elem {
                    crate::interp::funcs::Elem::One(v) => v,
                    crate::interp::funcs::Elem::Pair(a, b) => a.join(&b),
                })
            }
        }
    }
    let args: &[Value] = &args_full;
    {
        let specs = dc_specs(an, cls);
        let mut pos = 0usize;
        let n_positional = specs.iter().filter(|f| f.init && !f.kw_only).count();
        if args.len() > n_positional {
            // more positional arguments than the __init__ takes
            an.raise_here("TypeError", state);
            return;
        }
        for spec in specs.iter() {
            let (fname, default) = (&spec.name, spec.default);
            let fk = key_str_lit(fname);
            let mut v: Option<Value> = None;
            if !spec.init {
                // `field(init=False)`: its default
                let d = default.map(|d| dataclass_default(an, d, node, state)).unwrap_or_default();
                if !d.empty() {
                    let r = state.row(inst, ns).store(fk, true);
                    state.set_row(inst, ns, r);
                    state.set_slot(inst, ns, fk, d, true);
                }
                continue;
            }
            let positional = !spec.kw_only && pos < args.len();
            if !spec.kw_only {
                pos += 1;
            }
            if positional {
                v = Some(args[pos - 1].clone());
            } else if let Some(kv) = kwargs.get(&s(fname)) {
                v = Some(kv.clone());
            } else if let (Some(rest), false) = (&star_rest, spec.kw_only) {
                // (from the sequence of unknown length, or the default when it is short)
                let r = if rest.empty() { Value::data("field") } else { rest.clone() };
                v = Some(match default {
                    Some(d) => dataclass_default(an, d, node, state).join(&r),
                    None => r,
                });
            } else if !star_kwargs.is_empty() {
                // `**data` supplies the field when it has the key (over the default); a
                // field without a default must come from it: a read (a requirement on an
                // open input)
                let present = star_kwargs.iter().any(|sk| sk.pts.iter().any(|x| state.row(x, Ns::Item).present.contains(fk)));
                let definite = star_kwargs.iter().all(|sk| !sk.pts.is_empty() && sk.pts.iter().all(|x| state.row(x, Ns::Item).definite.contains(fk)));
                // a parameter's mapping: whether it has the key is known at the call site
                let mut acc = join_values(star_kwargs.iter().flat_map(|sk| sk.pts.iter().map(|x| state.slot(x, Ns::Item, fk)).collect::<Vec<_>>()));
                match default {
                    Some(d) if !definite => {
                        let dv = dataclass_default(an, d, node, state);
                        acc = if present { acc.join(&dv) } else { dv.join(&acc) };
                    }
                    Some(_) => {}
                    None if !definite => {
                        for sk in star_kwargs {
                            let recs: Vec<u32> = an.record_objects(sk).into_iter().filter(|o| state.row(*o, Ns::Item).open()).collect();
                            if !recs.is_empty() {
                                let lv = an.load_item(&Value::with_kv(crate::domain::ObjSet::from_vec(recs), sk.kv.clone()), Ns::Item, &Value::lit(fk), node, state, true);
                                acc = acc.join(&lv);
                            }
                        }
                    }
                    None => {}
                }
                v = Some(acc);
            } else if let Some(d) = default {
                v = Some(dataclass_default(an, d, node, state));
            }
            if v.is_none() && star_kwargs.is_empty() {
                if !ci.lib.typed_dict {
                    an.verdict(node, fk, &Classification::with_note(Tier::Missing, "is a dataclass field that was not given"), inst, ns, None, None);
                }
                continue;
            }
            let v = v.filter(|x| !x.empty()).unwrap_or_else(|| Value::data("field"));
            if spec.initvar {
                // an InitVar: an argument of __post_init__, no attribute
                initvars.push(v);
                continue;
            }
            let r = state.row(inst, ns).store(fk, true);
            state.set_row(inst, ns, r);
            state.set_slot(inst, ns, fk, v, true);
        }
        if ci.lib.typed_dict {
            for sk in star_kwargs {
                an.copy_into(inst, Ns::Item, sk, state, true, &KeySet::new());
            }
        }
    }
    // the generated __init__ ends by calling __post_init__ when the class defines one
    if let Some(pc) = an.class_lookup(cls, "__post_init__") {
        let fv = an.heap.cls_slots.get(&(pc, Ns::Attr, key_str_lit("__post_init__"))).cloned().unwrap_or_default();
        for f in fv.pts.iter() {
            if an.heap.kind(f) == Kind::Function {
                an.call_user(f, Some(Value::obj(inst)), initvars.clone(), IndexMap::new(), Vec::new(), Vec::new(), state, node, false);
            }
        }
    }
}

/// The fields of a dataclass (TypedDict / NamedTuple) as its `__init__` takes them: bases
/// first; `InitVar`s (init-only), `field(init=False)`, keyword-only ones (after `_: KW_ONLY`,
/// `field(kw_only=True)`, `@dataclass(kw_only=True)`).
pub fn dc_specs(an: &Analyzer, cls: u32) -> Vec<DcSpec> {
    let mut out: Vec<DcSpec> = Vec::new();
    for c in an.heap.mro(cls).into_iter().rev() {
        let Some(ci) = an.heap.class_info_opt(c) else { continue };
        if !ci.has_fields() {
            continue;
        }
        let attrs: Vec<(String, Option<&'static ast::Expr>)> = ci.fields();
        let mut kw_only = ci.dc_kw_only.get();
        for st in &ci.node.body {
            let ast::Stmt::AnnAssign(a) = st else { continue };
            let ast::Expr::Name(n) = &*a.target else { continue };
            let ann_name = match &*a.annotation {
                ast::Expr::Subscript(sb) => crate::interp::dotted_last(&sb.value),
                other => crate::interp::dotted_last(other),
            };
            if ann_name == "KW_ONLY" {
                kw_only = true;
                continue;
            }
            let initvar = ann_name == "InitVar";
            let name = n.id.to_string();
            if !initvar && !attrs.iter().any(|(f, _)| *f == name) {
                continue;
            }
            let default: Option<&'static ast::Expr> = a.value.as_deref();
            let mut init = true;
            let mut fkw = kw_only;
            if let Some(ast::Expr::Call(fc)) = default {
                if crate::interp::dotted_last(&fc.func) == "field" {
                    for k in fc.arguments.keywords.iter() {
                        match (k.arg.as_ref().map(|x| x.as_str()), &k.value) {
                            (Some("init"), ast::Expr::BooleanLiteral(b)) => init = b.value,
                            (Some("kw_only"), ast::Expr::BooleanLiteral(b)) => fkw = b.value,
                            _ => {}
                        }
                    }
                }
            }
            let spec = DcSpec { name: name.clone(), default, init, initvar, kw_only: fkw };
            match out.iter_mut().find(|x| x.name == name) {
                Some(e) => *e = spec,
                None => out.push(spec),
            }
        }
    }
    // classes whose fields are not from a class body (generated / NamedTuple): all plain
    if out.is_empty() {
        out = dataclass_fields_mro(an, cls).into_iter().map(|(name, default)| DcSpec { name, default, init: true, initvar: false, kw_only: false }).collect();
    }
    out
}

/// The default of a dataclass field: `field(default_factory=f)` calls `f` (a fresh
/// container per construction site), `field(default=x)` and plain defaults evaluate.
pub fn dataclass_default(an: &mut Analyzer, d: &'static ast::Expr, node: &Node, state: &mut State) -> Value {
    // the default's names are the defining module's (a class from another module, a
    // generated namedtuple): evaluated there
    let here = an.heap.module_info(an.frame().module).idx;
    let owner = crate::frontend::module_of_addr(d as *const ast::Expr as usize);
    if let Some(idx) = owner.filter(|i| *i != here) {
        if let Some(moid) = an.module_of_idx(idx) {
            let mi = an.heap.module_info(moid);
            let (ctx, single) = (an.frame().ctx.clone(), an.frame().single);
            return an.in_frame(Frame::new(FrameKind::Module, mi.scopes.module(), moid, &ctx, single, "<default>"), |an| dataclass_default_here(an, d, node, state));
        }
    }
    dataclass_default_here(an, d, node, state)
}

fn dataclass_default_here(an: &mut Analyzer, d: &'static ast::Expr, node: &Node, state: &mut State) -> Value {
    if let ast::Expr::Call(c) = d {
        if crate::interp::dotted_last(&c.func) == "field" {
            for kw in c.arguments.keywords.iter() {
                let Some(arg) = &kw.arg else { continue };
                match arg.id.as_str() {
                    "default_factory" => {
                        let f = an.eval(&kw.value, state);
                        return an.call_value(&f, node, None, state, Some(vec![]));
                    }
                    "default" => return an.eval(&kw.value, state),
                    _ => {}
                }
            }
            return Value::data("field");
        }
    }
    an.eval(d, state)
}

/// Dataclass fields in constructor order: base classes' fields first (MRO, nearest last).
pub fn dataclass_fields_mro(an: &Analyzer, cls: u32) -> Vec<(String, Option<&'static ast::Expr>)> {
    let mut out: Vec<(String, Option<&'static ast::Expr>)> = Vec::new();
    for c in an.heap.mro(cls).into_iter().rev() {
        if let Some(ci) = an.heap.class_info_opt(c) {
            for (name, d) in ci.fields() {
                if let Some(e) = out.iter_mut().find(|(n, _)| *n == name) {
                    e.1 = d;
                } else {
                    out.push((name, d));
                }
            }
        }
    }
    out
}

/// An ABC with abstract methods no class of its MRO implements: `cls()` raises TypeError.
pub fn abstract_unimplemented(an: &Analyzer, cls: u32) -> bool {
    let mro = an.heap.mro(cls);
    let is_abc = mro.iter().any(|c| {
        an.heap.class_info_opt(*c).map(|ci| {
            ci.node.arguments.as_ref().map(|a| a.args.iter().any(|b| crate::interp::dotted_last(b) == "ABC") || a.keywords.iter().any(|k| k.arg.as_ref().map(|x| x.as_str()) == Some("metaclass") && crate::interp::dotted_last(&k.value) == "ABCMeta")).unwrap_or(false)
        }).unwrap_or(false)
    });
    if !is_abc {
        return false;
    }
    // the first definition of each name along the MRO decides
    let mut seen: Vec<String> = Vec::new();
    for c in mro.iter() {
        let Some(ci) = an.heap.class_info_opt(*c) else { continue };
        for st in &ci.node.body {
            let (name, decos) = match st {
                ast::Stmt::FunctionDef(f) => (f.name.id.to_string(), &f.decorator_list),
                ast::Stmt::Assign(a) => {
                    for t in &a.targets {
                        if let ast::Expr::Name(n) = t {
                            seen.push(n.id.to_string());
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            if seen.contains(&name) {
                continue;
            }
            seen.push(name);
            if decos.iter().any(|d| crate::interp::dotted_last(&d.expression) == "abstractmethod" || matches!(&d.expression, ast::Expr::Call(cl) if crate::interp::dotted_last(&cl.func) == "abstractmethod")) {
                return true;
            }
        }
    }
    false
}

/// `super().__init__(…)` / `Base.__init__(self, …)` reaching a dataclass base: its generated
/// `__init__` on the instance (`dcinit.<class>` bound, `dcinit.class.<class>` called with self).
pub fn m_dcinit(an: &mut crate::interp::Analyzer, st: &mut State, c: &crate::interp::Call) -> Value {
    let rest = c.qual.strip_prefix("dcinit.").unwrap_or("");
    let (cls, self_v, args): (u32, Value, Vec<Value>) = match rest.strip_prefix("class.") {
        Some(cl) => (cl.parse().unwrap_or(0), c.args.first().cloned().unwrap_or_default(), c.args.iter().skip(1).cloned().collect()),
        None => (rest.parse().unwrap_or(0), c.recv.clone().unwrap_or_default(), c.args.clone()),
    };
    if cls != 0 {
        for o in self_v.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Instance).collect::<Vec<_>>() {
            dataclass_init(an, cls, o, &args, &c.star_args, &c.kwargs, &c.star_kwargs, st, &c.node);
        }
    }
    an.none_v.clone()
}

/// The decorators of a class the program defines that the models know: `total_ordering`, and
/// the dataclass ones (`@dataclass`, `attrs.define`, …) with their `frozen` / `kw_only` /
/// `order` options. True: a dataclass.
pub fn class_decorators(an: &mut Analyzer, c: &'static ast::StmtClassDef, coid: ObjId, info: &crate::interp::ClassInfo, state: &mut State) -> bool {
    let mut is_dataclass = false;
    for d in &c.decorator_list {
        let Some(q) = an.decorator_qualname(&d.expression, state) else { continue };
        if q == "functools.total_ordering" {
            an.m.total_ordering.insert(coid);
        }
        if q.ends_with("dataclass") || q.ends_with("attrs.define") || q.ends_with("attr.s") {
            is_dataclass = true;
            if q.starts_with("pydantic") {
                info.pyd_dataclass.set(true);
            }
            if let ast::Expr::Call(dc) = &d.expression {
                for k in dc.arguments.keywords.iter() {
                    match (k.arg.as_ref().map(|x| x.as_str()), &k.value) {
                        (Some("frozen"), ast::Expr::BooleanLiteral(b)) => info.dc_frozen.set(b.value),
                        (Some("kw_only"), ast::Expr::BooleanLiteral(b)) => info.dc_kw_only.set(b.value),
                        (Some("order"), ast::Expr::BooleanLiteral(b)) => info.dc_order.set(b.value),
                        _ => {}
                    }
                }
            }
        }
    }
    is_dataclass
}

/// The fields a dataclass-like class declares (annotated names in its body, with their
/// defaults): not a `ClassVar`, `InitVar` or `KW_ONLY`, nor pydantic's private `_x` /
/// `model_config`.
pub fn declared_fields(c: &'static ast::StmtClassDef, pydantic: bool) -> Vec<(String, Option<&'static ast::Expr>)> {
    let mut fields = Vec::new();
    for st in &c.body {
        let ast::Stmt::AnnAssign(a) = st else { continue };
        let ast::Expr::Name(n) = &*a.target else { continue };
        let ann = |name: &str, sub: bool| match &*a.annotation {
            ast::Expr::Subscript(sb) => sub && crate::interp::dotted_last(&sb.value) == name,
            other => !sub && crate::interp::dotted_last(other) == name,
        };
        let skipped = ann("ClassVar", true) || ann("ClassVar", false) || ann("InitVar", true) || ann("KW_ONLY", false);
        if skipped || (pydantic && (n.id.starts_with('_') || n.id.as_str() == "model_config")) {
            continue;
        }
        fields.push((n.id.to_string(), a.value.as_deref()));
    }
    fields
}

/// A class whose hierarchy includes a `NamedTuple` class.
pub fn is_named_tuple_class(an: &Analyzer, cls: u32) -> bool {
    let mut work = vec![cls];
    let mut seen: Vec<u32> = Vec::new();
    while let Some(c) = work.pop() {
        if seen.contains(&c) {
            continue;
        }
        seen.push(c);
        let Some(ci) = an.heap.class_info_opt(c) else { continue };
        if ci.lib.named_tuple {
            return true;
        }
        work.extend(ci.bases.iter().copied());
    }
    false
}

/// A NamedTuple instance indexed by position (from either end; an unknown index: any
/// field, or a slice of them).
pub fn named_tuple_item(an: &mut Analyzer, oid: u32, cls: u32, key: &Value, single: bool, node: &Node, state: &mut State) -> Value {
    let fs = dataclass_fields_mro(an, cls);
    let n = fs.len() as i64;
    let field = |i: i64, state: &State| state.slot(oid, Ns::Attr, key_str_lit(&fs[(if i < 0 { n + i } else { i }) as usize].0));
    match key.kv.as_lit().filter(|_| key.pts.is_empty()).and_then(intern::key_as_int) {
        Some(i) if -n <= i && i < n => field(i, state),
        Some(_) if single => {
            an.raise_here("IndexError", state);
            Value::new()
        }
        _ => {
            an.may_raise_here("IndexError", state);
            let mut parts: Vec<Value> = (0..n).map(|i| field(i, state)).collect();
            parts.push(Value::data(&format!("L{}", node.line)));
            crate::domain::join_values(parts)
        }
    }
}
/// A TypedDict's required and optional keys (`total=`, `Required[…]` / `NotRequired[…]`,
/// inherited from TypedDict bases).
pub fn typed_dict_keys(an: &Analyzer, cls: u32) -> Option<(KeySet, KeySet)> {
    let ci = an.heap.class_info_opt(cls)?;
    if !ci.lib.typed_dict {
        return None;
    }
    let (mut req, mut opt) = (KeySet::new(), KeySet::new());
    for b in ci.bases.iter() {
        if let Some((r, o)) = typed_dict_keys(an, *b) {
            req = req.union(&r);
            opt = opt.union(&o);
        }
    }
    let c = ci.node;
    let total = c.arguments.as_ref().and_then(|a| a.keywords.iter().find(|k| k.arg.as_ref().map(|x| x.as_str()) == Some("total"))).map(|k| !matches!(&k.value, ast::Expr::BooleanLiteral(b) if !b.value)).unwrap_or(true);
    let last = |e: &ast::Expr| -> String {
        match e {
            ast::Expr::Name(n) => n.id.to_string(),
            ast::Expr::Attribute(a) => a.attr.id.to_string(),
            _ => String::new(),
        }
    };
    for st in &c.body {
        if let ast::Stmt::AnnAssign(a) = st {
            if let ast::Expr::Name(n) = &*a.target {
                let mut required = total;
                let mut ann: &ast::Expr = &a.annotation;
                // `Annotated[Required[int], …]`
                loop {
                    let ast::Expr::Subscript(sb) = ann else { break };
                    match last(&sb.value).as_str() {
                        "Required" => required = true,
                        "NotRequired" => required = false,
                        "Annotated" | "ReadOnly" => {}
                        _ => break,
                    }
                    ann = match &*sb.slice {
                        ast::Expr::Tuple(t) if !t.elts.is_empty() => &t.elts[0],
                        other => other,
                    };
                }
                let k = intern::key_str_lit(&n.id);
                if required {
                    req = req.union(&KeySet::one(k));
                    opt = opt.difference(&KeySet::one(k));
                } else {
                    opt = opt.union(&KeySet::one(k));
                    req = req.difference(&KeySet::one(k));
                }
            }
        }
    }
    Some((req, opt))
}

/// The dataclass whose generated `__init__` `cls` inherits (none when a user `__init__`
/// comes first in the MRO).
pub fn dataclass_init_owner(an: &Analyzer, cls: u32) -> Option<u32> {
    for c in an.heap.mro(cls) {
        if an.heap.cls_slots.contains_key(&(c, Ns::Attr, key_str_lit("__init__"))) {
            return None;
        }
        if an.heap.class_info_opt(c).map(|x| x.has_fields()).unwrap_or(false) {
            return Some(c);
        }
    }
    None
}

/// The key a frozen dataclass / pydantic instance hashes as (its fields), when known.
pub fn frozen_instance_key(an: &Analyzer, o: u32, state: &State, depth: usize) -> Option<KeyId> {
    let c = an.heap.instance_class(o)?;
    let ci = an.heap.class_info_opt(c)?;
    // a frozen pydantic model hashes by its fields too
    let pyd_fields: Option<Vec<(String, Option<&'static ast::Expr>)>> = an.m.pyd_classes.get(&c).filter(|p| p.frozen).map(|p| p.fields.iter().map(|f| (f.name.clone(), None)).collect());
    if (!ci.dc_frozen.get() && pyd_fields.is_none()) || an.class_lookup(c, "__hash__").is_some() || an.class_lookup(c, "__eq__").is_some() {
        return None;
    }
    let fields = match pyd_fields {
        Some(f) => f,
        None => ci.dataclass_fields.borrow().clone()?,
    };
    let row = state.row(o, Ns::Attr);
    let mut parts = Vec::new();
    for (f, _) in &fields {
        let fk = intern::key_str_lit(f);
        if !row.definite.contains(fk) {
            return None;
        }
        let v = state.slot(o, Ns::Attr, fk);
        let k = match (v.pts.is_empty(), v.kv.as_lit(), v.single()) {
            (true, Some(k), _) => k,
            (false, _, Some(x)) if v.kv.is_bot() => an.value_key(x, state, depth + 1)?,
            _ => return None,
        };
        parts.push(format!("{}={}", f, intern::key_repr(k)));
    }
    let name = an.heap.obj(c).label.trim_start_matches("class ").to_string();
    let k = intern::key_member(&format!("{}({})", name, parts.join(", ")), None);
    an.heap.keys.identity_objs.borrow_mut().entry(k).or_insert(o);
    Some(k)
}

/// Two instances of an `@dataclass(order=True)` class: their fields (those not
/// `field(compare=False)`) compared as tuples, when known.
pub fn dataclass_order(an: &Analyzer, x: u32, y: u32, state: &State) -> Option<std::cmp::Ordering> {
    let cx = an.heap.instance_class(x)?;
    if Some(cx) != an.heap.instance_class(y) || !an.heap.class_info_opt(cx).map(|ci| ci.dc_order.get()).unwrap_or(false) {
        return None;
    }
    let compared = |d: &Option<&'static ast::Expr>| match d {
        Some(ast::Expr::Call(fc)) if crate::interp::dotted_last(&fc.func) == "field" => !fc.arguments.keywords.iter().any(|k| k.arg.as_ref().map(|n| n.as_str()) == Some("compare") && matches!(&k.value, ast::Expr::BooleanLiteral(bl) if !bl.value)),
        _ => true,
    };
    let fields: Vec<KeyId> = dataclass_fields_mro(an, cx).iter().filter(|(_, d)| compared(d)).map(|(f, _)| key_str_lit(f)).collect();
    let (rx, ry) = (state.row(x, Ns::Attr), state.row(y, Ns::Attr));
    if !fields.iter().all(|f| rx.definite.contains(*f) && ry.definite.contains(*f)) {
        return None;
    }
    for f in &fields {
        match crate::models::stdfuncs::value_order(an, state, &state.slot(x, Ns::Attr, *f), &state.slot(y, Ns::Attr, *f)) {
            Some(std::cmp::Ordering::Equal) => continue,
            o => return o,
        }
    }
    Some(std::cmp::Ordering::Equal)
}

/// A TypedDict class called: a plain dict.
#[allow(clippy::too_many_arguments)]
pub fn typed_dict_construct(an: &mut Analyzer, cls: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Option<Value> {
    if !an.heap.class_info(cls).lib.typed_dict {
        return None;
    }
    let call = crate::interp::Call { qual: "builtins.dict".into(), recv: None, args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
    let m = an.model("builtins.dict")?;
    Some(m(an, state, &call))
}

/// A library class's instance `inst` of `cls` made: a pydantic model (without its own
/// `__init__`) validates its keyword input; a dict subclass without its own `__init__` is
/// filled by dict's constructor; a dataclass (or a subclass without its own `__init__`)
/// runs the generated `__init__`. None: none of these.
#[allow(clippy::too_many_arguments)]
pub fn construct_library(an: &mut Analyzer, cls: u32, inst: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Option<Value> {
    let ci = an.heap.class_info(cls);
    if crate::models::pydantic::is_validated(an, cls) {
        let own_init = an.class_lookup(cls, "__init__").map(|c| crate::models::pydantic::is_validated(an, c) && an.heap.cls_slots.contains_key(&(c, Ns::Attr, key_str_lit("__init__")))).unwrap_or(false);
        if !own_init {
            // a pydantic dataclass takes its fields positionally too
            let mut kw = kwargs.clone();
            if !crate::models::pydantic::is_model(an, cls) {
                let names: Vec<String> = dataclass_fields_mro(an, cls).into_iter().map(|(n, _)| n).collect();
                for (i, a) in args.iter().enumerate() {
                    if let Some(n) = names.get(i) {
                        kw.insert(intern::s(n), a.clone());
                    }
                }
            }
            return Some(crate::models::pydantic::pyd_construct(an, cls, args, &kw, star_kwargs, state, node));
        }
    }
    let dict_based = an.heap.mro(cls).iter().any(|c| an.heap.class_info_opt(*c).map(|x| x.lib.dict_base).unwrap_or(false));
    if dict_based && an.class_lookup(cls, "__init__").is_none() {
        state.set_row(inst, Ns::Item, crate::domain::Row::new());
        if let Some(a0) = args.first() {
            if !an.record_objects(a0).is_empty() {
                an.copy_into(inst, Ns::Item, a0, state, true, &KeySet::new());
            } else {
                let r = state.row(inst, Ns::Item).data_store(crate::domain::Source::new(&format!("{}@L{}", ci.name, node.line), crate::domain::SourceKind::Data));
                state.set_row(inst, Ns::Item, r);
            }
        }
        for (k, v) in kwargs.iter() {
            an.store_key(inst, Ns::Item, &crate::domain::KeyVal::Lit(intern::key(intern::Key::Str(intern::str_get(*k)))), v, true, node, state);
        }
        for sk in star_kwargs {
            an.copy_into(inst, Ns::Item, sk, state, true, &KeySet::new());
        }
        return Some(Value::obj(inst));
    }
    // the generated `__init__` of a dataclass base applies to a subclass without its own
    let owner = an.heap.mro(cls).into_iter().find_map(|c| if an.heap.cls_slots.contains_key(&(c, Ns::Attr, key_str_lit("__init__"))) { Some(false) } else if an.heap.class_info_opt(c).is_some_and(|x| x.has_fields()) { Some(true) } else { None });
    if ci.has_fields() || owner == Some(true) {
        dataclass_init(an, cls, inst, args, star_args, kwargs, star_kwargs, state, node);
        return Some(Value::obj(inst));
    }
    None
}

/// An Exception subclass without its own `__init__` made: its `args` are the arguments.
pub fn exception_init(an: &mut Analyzer, cls: u32, inst: u32, args: &[Value], node: &Node, state: &mut State) {
    let name = an.heap.class_info(cls).name.clone();
    let t = an.alloc_positional(Kind::Tuple, node, Some(args.to_vec()), Value::new(), crate::domain::DATA, state);
    let strong = an.heap.obj(inst).single;
    an.store_key(inst, Ns::Attr, &crate::domain::KeyVal::Lit(key_str_lit("args")), &t, strong, node, state);
    an.m.exc_objs.insert(inst, name);
}
