//! `dataclasses` (and the classes built like them: NamedTuple, TypedDict): the generated
//! `__init__` (fields in MRO order, defaults, `field(...)`, `InitVar`, `KW_ONLY`,
//! `__post_init__`).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, Classification, Ns, ObjId, Tier, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeySet, StrId};
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

impl Analyzer {
    /// The generated `__init__` of a dataclass / TypedDict / NamedTuple `cls` run on `inst`
    /// (a fresh instance, or `self` when called explicitly as `Base.__init__(self, …)` /
    /// `super().__init__(…)` from a subclass).
    pub fn dataclass_init(&mut self, cls: u32, inst: ObjId, args: &[Value], star_args: &[Value], kwargs: &IndexMap<StrId, Value>, star_kwargs: &[Value], state: &mut State, node: &Node) {
        let ci = self.class_info(cls);
        let ns = if ci.typed_dict { Ns::Item } else { Ns::Attr };
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
            if let Some(o) = sa.single().filter(|o| matches!(self.kind(*o), Kind::Tuple | Kind::List) && sa.kv.is_bot()) {
                if let Some(n) = self.llen(o, state) {
                    for i in 0..n {
                        args_full.push(state.slot(o, Ns::Pos, intern::key_int(i as i64)));
                    }
                    continue;
                }
            }
            let it = self.iterate(sa, node, state);
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
            let specs = self.dc_specs(cls);
            let mut pos = 0usize;
            let n_positional = specs.iter().filter(|f| f.init && !f.kw_only).count();
            if args.len() > n_positional {
                // more positional arguments than the __init__ takes
                self.raise_here("TypeError", state);
                return;
            }
            for spec in specs.iter() {
                let (fname, default) = (&spec.name, spec.default);
                let fk = key_str_lit(fname);
                let mut v: Option<Value> = None;
                if !spec.init {
                    // `field(init=False)`: its default
                    let d = default.map(|d| self.dataclass_default(d, node, state)).unwrap_or_default();
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
                        Some(d) => self.dataclass_default(d, node, state).join(&r),
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
                            let dv = self.dataclass_default(d, node, state);
                            acc = if present { acc.join(&dv) } else { dv.join(&acc) };
                        }
                        Some(_) => {}
                        None if !definite => {
                            for sk in star_kwargs {
                                let recs: Vec<u32> = self.record_objects(sk).into_iter().filter(|o| state.row(*o, Ns::Item).open()).collect();
                                if !recs.is_empty() {
                                    let lv = self.load_item(&Value::with_kv(crate::domain::ObjSet::from_vec(recs), sk.kv.clone()), Ns::Item, &Value::lit(fk), node, state, true);
                                    acc = acc.join(&lv);
                                }
                            }
                        }
                        None => {}
                    }
                    v = Some(acc);
                } else if let Some(d) = default {
                    v = Some(self.dataclass_default(d, node, state));
                }
                if v.is_none() && star_kwargs.is_empty() {
                    if !ci.typed_dict {
                        self.verdict(node, fk, &Classification::with_note(Tier::Missing, "dataclass field not supplied"), inst, ns, None, None);
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
            if ci.typed_dict {
                for sk in star_kwargs {
                    self.copy_into(inst, Ns::Item, sk, state, true, &KeySet::new());
                }
            }
        }
        // the generated __init__ ends by calling __post_init__ when the class defines one
        if let Some(pc) = self.class_lookup(cls, "__post_init__") {
            let fv = self.cls_slots.get(&(pc, Ns::Attr, key_str_lit("__post_init__"))).cloned().unwrap_or_default();
            for f in fv.pts.iter() {
                if self.kind(f) == Kind::Function {
                    self.call_user(f, Some(Value::obj(inst)), initvars.clone(), IndexMap::new(), Vec::new(), Vec::new(), state, node, None, false);
                }
            }
        }
    }

    /// The fields of a dataclass (TypedDict / NamedTuple) as its `__init__` takes them: bases
    /// first; `InitVar`s (init-only), `field(init=False)`, keyword-only ones (after `_: KW_ONLY`,
    /// `field(kw_only=True)`, `@dataclass(kw_only=True)`).
    pub fn dc_specs(&self, cls: u32) -> Vec<DcSpec> {
        let mut out: Vec<DcSpec> = Vec::new();
        for c in self.mro(cls).into_iter().rev() {
            let Some(ci) = self.class_info_opt(c) else { continue };
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
            out = self.dataclass_fields_mro(cls).into_iter().map(|(name, default)| DcSpec { name, default, init: true, initvar: false, kw_only: false }).collect();
        }
        out
    }

    /// The default of a dataclass field: `field(default_factory=f)` calls `f` (a fresh
    /// container per construction site), `field(default=x)` and plain defaults evaluate.
    pub fn dataclass_default(&mut self, d: &'static ast::Expr, node: &Node, state: &mut State) -> Value {
        // the default's names are the defining module's (a class from another module, a
        // generated namedtuple): evaluated there
        let here = self.module_info(self.frame().module).idx;
        let owner = crate::frontend::module_of_addr(d as *const ast::Expr as usize);
        if let Some(idx) = owner.filter(|i| *i != here) {
            if let Some(moid) = self.modules.values().copied().find(|m| self.module_info(*m).idx == idx) {
                let mi = self.module_info(moid);
                let (ctx, single) = (self.frame().ctx.clone(), self.frame().single);
                self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), moid, &ctx, single, "<default>"));
                let v = self.dataclass_default_here(d, node, state);
                self.frames.pop();
                return v;
            }
        }
        self.dataclass_default_here(d, node, state)
    }

    fn dataclass_default_here(&mut self, d: &'static ast::Expr, node: &Node, state: &mut State) -> Value {
        if let ast::Expr::Call(c) = d {
            if crate::interp::dotted_last(&c.func) == "field" {
                for kw in c.arguments.keywords.iter() {
                    let Some(arg) = &kw.arg else { continue };
                    match arg.id.as_str() {
                        "default_factory" => {
                            let f = self.eval(&kw.value, state);
                            return self.call_value(&f, node, None, state, Some(vec![]));
                        }
                        "default" => return self.eval(&kw.value, state),
                        _ => {}
                    }
                }
                return Value::data("field");
            }
        }
        self.eval(d, state)
    }

    /// Dataclass fields in constructor order: base classes' fields first (MRO, nearest last).
    pub fn dataclass_fields_mro(&self, cls: u32) -> Vec<(String, Option<&'static ast::Expr>)> {
        let mut out: Vec<(String, Option<&'static ast::Expr>)> = Vec::new();
        for c in self.mro(cls).into_iter().rev() {
            if let Some(ci) = self.class_info_opt(c) {
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
}
