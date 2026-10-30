//! Class definitions: the class object, its body run in a class frame, its bases and what
//! library bases make of it (Enum, dataclass, pydantic, NamedTuple, TypedDict, nn.Module).

use ruff_python_ast as ast;

use crate::domain::{ObjId, Ns, Value};
use crate::intern::KeyId;
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s};
use crate::state::{Kind, State};

use super::{Analyzer, Args, ClassInfo, Frame, FrameKind, Payload};

/// The names CPython puts in a class namespace beside the body's (`class_namespace`).
const NS_ADDED: &[&str] = &["__module__", "__qualname__", "__firstlineno__", "__doc__", "__static_attributes__", "__annotate_func__", "__classdictcell__", "__classcell__"];

impl Analyzer {
    /// A `class` statement: its bases (library ones say what it is), the class object, its body
    /// run in a class frame (names become class attributes), then its decorators and what its
    /// library bases make of it (enum members, dataclass fields, pydantic, NamedTuple, …).
    pub(super) fn stmt_classdef(&mut self, c: &'static ast::StmtClassDef, state: &mut State) {
        let fr_module = self.frame().module;
        let fr_ctx = self.frame().ctx.clone();
        let fr_single = self.frame().single;
        let fr_cells = self.frame().cells.clone();
        let (bases, mut open_base, lib, ek, own_meta, base_vals) = self.class_bases(c, state);
        // (its metaclass: the one it names, else its first base's that has one)
        let meta = own_meta.or_else(|| bases.iter().find_map(|b| self.heap.class_info_opt(*b).and_then(|ci| ci.meta.clone())));
        open_base |= lib.open;
        let mut lek = lib.enum_kind;
        lek.join(ek);
        let (is_enum, str_mixin) = (lib.is_enum, lib.str_mixin);
        let (typed_dict, named_tuple, pydantic) = (lib.typed_dict, lib.named_tuple, lib.pydantic);
        let info = ClassInfo {
            name: c.name.id.to_string(),
            node: c,
            bases,
            module: fr_module,
            open_base: std::cell::Cell::new(open_base),
            dataclass_fields: std::cell::RefCell::new(None),
            lib,
            pyd_dataclass: std::cell::Cell::new(false),
            dc_frozen: std::cell::Cell::new(false),
            dc_kw_only: std::cell::Cell::new(false),
            dc_order: std::cell::Cell::new(false),
            any_fields: std::cell::RefCell::new(None),
            meta: meta.clone(),
        };
        let info = std::rc::Rc::new(info);
        let cnode = frontend::node(c);
        let coid = self.alloc_at(Kind::Class, &cnode, state, &format!("class {}", c.name.id), Payload::Class(info.clone()), None, Ns::Attr, None, None);
        self.program.user_classes.push(coid);
        let is_dataclass = crate::models::dataclasses::class_decorators(self, c, coid, &info, state);
        if is_dataclass || typed_dict || named_tuple || pydantic {
            *info.dataclass_fields.borrow_mut() = Some(crate::models::dataclasses::declared_fields(c, pydantic));
        }
        // class body
        let mi = self.heap.module_info(fr_module);
        let cscope = mi.scopes.of_node(crate::scopes::node_addr(c)).unwrap_or_else(|| mi.scopes.module());
        let fr_single_nl = self.frame().single_nl;
        let mut cf = Frame::new(FrameKind::Class, cscope, fr_module, &fr_ctx, fr_single, &format!("class {}", c.name.id));
        cf.single_nl = fr_single_nl;
        cf.class_oid = Some(coid);
        cf.cells = fr_cells;
        self.cx.frames.push(cf);
        for st in &c.body {
            if let ast::Stmt::AnnAssign(a) = st {
                if a.value.is_none() {
                    continue;
                }
            }
            self.exec_stmt(st, state);
        }
        self.cx.frames.pop();
        let enum_base = info.bases.iter().any(|b| self.m.enum_members.contains_key(b));
        if is_enum || enum_base {
            crate::models::enums::make_enum_members(self, c, coid, lek, str_mixin, state);
        }
        // (its metaclass, named by it or a base: the program's runs as Python runs it, given
        // the namespace the body filled; another is code rowan does not run, which may rebind
        // the data the body defined)
        let run_meta = meta.as_ref().map(|m| self.program_metaclass(m));
        let ns = match run_meta {
            Some(Some(Some(_))) => Some(self.class_namespace(c, coid, &cnode.sub(0x6d6e), state)),
            _ => None,
        };
        self.run_set_name(c, coid, &cnode, state);
        self.run_init_subclass(c, coid, &info, &cnode, state);
        let mut cv = Value::obj(coid);
        match (run_meta, ns) {
            (Some(Some(Some(m))), Some(ns)) => cv = self.run_metaclass(c, m, coid, ns, &base_vals, &cnode, state),
            (Some(None), _) => self.metaclass_not_run(meta.as_ref().unwrap(), coid, &cnode, state),
            _ => {}
        }
        if !is_dataclass && !c.decorator_list.is_empty() {
            let dv = self.apply_decorators(&c.decorator_list, cnode, cv.clone(), state);
            if !dv.pts.is_empty() {
                cv = dv;
            }
        }
        self.store_name(s(c.name.id.as_str()), cv, state);
    }

    /// `name = property(fget, fset, fdel, doc)` in a class body (`self.frame()` its frame): as
    /// the decorators make it — a copy of the getter that is a property, the setter and deleter
    /// registered under `name`. False (nothing done) for any other statement, or a getter, setter
    /// or deleter that is not one function of the program's (the `property` model's call then).
    pub(super) fn class_property(&mut self, a: &'static ast::StmtAssign, state: &mut State) -> bool {
        let ([ast::Expr::Name(target)], ast::Expr::Call(call)) = (&a.targets[..], &*a.value) else { return false };
        if call.arguments.args.iter().any(|e| matches!(e, ast::Expr::Starred(_))) || call.arguments.keywords.iter().any(|k| k.arg.is_none()) {
            return false;
        }
        if self.decorator_qualname(&call.func, state).as_deref() != Some("builtins.property") {
            return false;
        }
        // (positional or by keyword: fget, fset, fdel, doc)
        let arg = |i: usize, kw: &str| call.arguments.args.get(i).or_else(|| call.arguments.keywords.iter().find(|k| k.arg.as_ref().is_some_and(|n| n.id.as_str() == kw)).map(|k| &k.value));
        let [fget, fset, fdel, doc] = [(0, "fget"), (1, "fset"), (2, "fdel"), (3, "doc")].map(|(i, kw)| arg(i, kw));
        let mut probe = state.copy();
        let one_function = |an: &mut Analyzer, e: Option<&'static ast::Expr>, st: &mut State| e.map(|e| an.eval(e, st)).and_then(|v| v.single().filter(|f| v.kv.is_bot() && an.heap.kind(*f) == Kind::Function));
        let Some(getter) = one_function(self, fget, &mut probe) else { return false };
        let (setter, deleter) = (one_function(self, fset, &mut probe), one_function(self, fdel, &mut probe));
        if setter.is_none() && fset.is_some() || deleter.is_none() && fdel.is_some() {
            return false;
        }
        if let Some(d) = doc {
            self.eval(d, &mut probe);
        }
        *state = probe;
        let cls = self.frame().class_oid.unwrap();
        let nk = key_str_lit(target.id.as_str());
        let fi = self.heap.func_info(getter);
        let info = super::FuncInfo {
            node: fi.node,
            name: fi.name.clone(),
            qualname: fi.qualname.clone(),
            module: fi.module,
            scope: fi.scope,
            closure: fi.closure.clone(),
            defaults: fi.defaults.clone(),
            cls: fi.cls,
            is_property: std::cell::Cell::new(true),
            is_static: std::cell::Cell::new(false),
            is_classmethod: std::cell::Cell::new(false),
            is_contextmanager: std::cell::Cell::new(false),
            is_cached: std::cell::Cell::new(false),
            is_memo: std::cell::Cell::new(fi.is_memo.get()),
            opaque: std::cell::Cell::new(fi.opaque.get()),
        };
        let prop = self.alloc_at(Kind::Function, &frontend::node(&*a.value), state, &format!("property {}", target.id), Payload::Func(std::rc::Rc::new(info)), Some(crate::domain::Row::new()), Ns::Attr, None, None);
        self.heap.prop_names.insert(nk);
        if let Some(f) = setter {
            self.heap.prop_setters.insert((cls, nk), f);
        }
        if let Some(f) = deleter {
            self.heap.prop_deleters.insert((cls, nk), f);
        }
        self.store_name(s(target.id.as_str()), Value::obj(prop), state);
        true
    }

    /// The bases a class statement names: the program's classes, whether the class is open
    /// (a base rowan does not see, a metaclass other than ABCMeta), what its library bases make
    /// of it, the Enum kind its Enum bases give, the metaclass it names (not ABCMeta), and the
    /// bases' values as written.
    #[allow(clippy::type_complexity)]
    fn class_bases(&mut self, c: &'static ast::StmtClassDef, state: &mut State) -> (Vec<u32>, bool, crate::models::ClassBase, crate::interp::EnumKind, Option<Value>, Vec<Value>) {
        let mut meta = None;
        let mut base_vals = Vec::new();
        let mut bases: Vec<u32> = Vec::new();
        let mut open_base = false;
        let mut lib = crate::models::ClassBase::default();
        let mut ek = crate::interp::EnumKind::default();
        if let Some(args) = &c.arguments {
            for b in args.args.iter() {
                // `RootModel[list[int]]`, `Generic[T]`: the subscripted class decides
                let b: &ast::Expr = match b {
                    ast::Expr::Subscript(sb) => &sb.value,
                    other => other,
                };
                let bv = self.eval(b, state);
                base_vals.push(bv.clone());
                for oid in bv.pts.iter() {
                    match self.heap.kind(oid) {
                        Kind::Class => {
                            if let Some(k) = self.m.enum_kind.get(&oid) {
                                ek.mixed |= k.mixed;
                                ek.is_str |= k.is_str;
                                ek.repr_value |= k.repr_value;
                                ek.flag |= k.flag;
                            }
                            // (a base that derives from an external class — `list`, an unseen
                            // library class — makes this one's instances open too)
                            if self.heap.class_info_opt(oid).is_some_and(|ci| ci.open_base.get()) {
                                open_base = true;
                            }
                            bases.push(oid)
                        }
                        Kind::External => lib.join(crate::models::class_base(&self.heap.external_qual(oid))),
                        _ => open_base = true,
                    }
                }
                if bv.pts.is_empty() {
                    open_base = true;
                }
            }
            // a metaclass other than ABCMeta may add anything to the class and its instances
            for kw in args.keywords.iter() {
                if kw.arg.as_ref().map(|a| a.as_str() == "metaclass").unwrap_or(false) {
                    let mv = self.eval(&kw.value, state);
                    let abc = mv.pts.iter().all(|o| self.heap.kind(o) == Kind::External && self.heap.external_qual(o).ends_with("ABCMeta")) && !mv.pts.is_empty();
                    // (one of the program's that rowan runs and that leaves making instances
                    // to `type`: as the class's code says)
                    if !abc {
                        open_base |= !matches!(self.program_metaclass(&mv), Some(Some(m)) if self.class_lookup(m, "__call__").is_none());
                        meta = Some(mv);
                    }
                }
            }
        }
        (bases, open_base, lib, ek, meta, base_vals)
    }

    /// `__set_name__(owner, name)` on the descriptor objects assigned in the class body.
    fn run_set_name(&mut self, c: &'static ast::StmtClassDef, coid: ObjId, cnode: &Node, state: &mut State) {
        for st in &c.body {
            let ast::Stmt::Assign(a) = st else { continue };
            let [ast::Expr::Name(n)] = &a.targets[..] else { continue };
            let v = self.heap.cls_slots.get(&(coid, Ns::Attr, key_str_lit(n.id.as_str()))).cloned().unwrap_or_default();
            for d in v.pts.iter() {
                // (a data descriptor takes reads over the instance's own value, as a property does)
                if self.is_descriptor(d, "__set__") || self.is_descriptor(d, "__delete__") {
                    self.heap.prop_names.insert(key_str_lit(n.id.as_str()));
                }
                if self.heap.kind(d) == Kind::Instance && self.heap.instance_class(d).and_then(|dc| self.class_lookup(dc, "__set_name__")).is_some() {
                    self.call_method_with(&Value::obj(d), "__set_name__", vec![Value::obj(coid), Value::lit(key_str_lit(n.id.as_str()))], cnode, state);
                }
            }
        }
    }

    /// A base's `__init_subclass__` runs for the new class (an implicit classmethod), given the
    /// class statement's keywords. A followed library's that does not complete (calls in it do
    /// not run: past budgets or its entry's allowance) may have given the class attributes of
    /// any name (`setattr` by computed names): its base is open, as an unseen one is.
    fn run_init_subclass(&mut self, c: &'static ast::StmtClassDef, coid: ObjId, info: &ClassInfo, cnode: &Node, state: &mut State) {
        let found = info.bases.iter().find_map(|b| self.class_lookup(*b, "__init_subclass__"));
        if let Some(defc) = found {
            let fv = self.heap.class_attr(defc, "__init_subclass__");
            let kwargs = self.class_keywords(c, state);
            let skipped = self.budget.skipped;
            let mut library = false;
            for f in fv.pts.iter() {
                if self.heap.kind(f) == Kind::Function {
                    library |= self.heap.module_info(self.heap.func_info(f).module).library;
                    // concretely: the class's own attributes (`cls.__name__`) are known here
                    self.call_user(f, Some(Value::obj(coid)), Args::kw(&kwargs), state, cnode, false);
                }
            }
            if library && self.budget.skipped > skipped {
                info.open_base.set(true);
            }
        }
    }

    /// The class statement's keywords other than `metaclass=`, evaluated.
    fn class_keywords(&mut self, c: &'static ast::StmtClassDef, state: &mut State) -> indexmap::IndexMap<intern::StrId, Value> {
        let mut kwargs = indexmap::IndexMap::new();
        for kw in c.arguments.iter().flat_map(|a| a.keywords.iter()) {
            if let Some(a) = kw.arg.as_ref().filter(|a| a.as_str() != "metaclass") {
                let v = self.eval(&kw.value, state);
                kwargs.insert(s(a.as_str()), v);
            }
        }
        kwargs
    }

    /// What runs of metaclass `mv`: Some(None) nothing of the program's (`type`, ABCMeta);
    /// Some(Some(m)) the program's class `m` (`is_metaclass`) without `__prepare__`; None
    /// anything else, which is code rowan does not run.
    fn program_metaclass(&self, mv: &Value) -> Option<Option<ObjId>> {
        let m = mv.single().filter(|_| mv.kv.is_bot())?;
        match self.heap.kind(m) {
            Kind::External if matches!(self.heap.external_qual(m).as_str(), "builtins.type" | "abc.ABCMeta") => Some(None),
            Kind::Class if self.is_metaclass(m) && self.class_lookup(m, "__prepare__").is_none() => Some(Some(m)),
            _ => None,
        }
    }

    /// Is `m` a class of the program's deriving from `type` (or ABCMeta) and from no other
    /// library class?
    pub fn is_metaclass(&self, m: ObjId) -> bool {
        let mut typed = false;
        for c in self.heap.mro(m) {
            let Some(ci) = self.heap.class_info_opt(c) else { return false };
            for q in &ci.lib.quals {
                match q.as_str() {
                    "builtins.type" | "abc.ABCMeta" => typed = true,
                    "builtins.object" => {}
                    _ => return false,
                }
            }
        }
        typed
    }

    /// A metaclass rowan does not run made the class: code given the metaclass and the class's
    /// data attributes, which it may rebind or change (the methods it keeps).
    fn metaclass_not_run(&mut self, meta: &Value, coid: ObjId, cnode: &Node, state: &mut State) {
        let any = Value::obj(super::any_object(&mut self.heap));
        let mut given = vec![meta.clone()];
        for (k, v) in state.obj_slots(coid, Ns::Attr) {
            if v.pts.iter().any(|o| self.holds_code(o)) {
                continue;
            }
            state.set_slot(coid, Ns::Attr, k, v.join(&any), true);
            given.push(v);
        }
        self.unknown_call("metaclass", None, Args::pos(&given), state, cnode);
    }

    /// The namespace the class body filled, as the dict its metaclass is given, in CPython
    /// 3.14's order: `__module__`, `__qualname__`, `__firstlineno__`, a docstring's `__doc__`,
    /// the body's names, then what the compiler adds (`syntax::class_namespace_cells`); pushed
    /// as the class the running statement makes (`Context::making_class`).
    fn class_namespace(&mut self, c: &'static ast::StmtClassDef, coid: ObjId, node: &Node, state: &mut State) -> ObjId {
        let ns = self.alloc_simple(Kind::Dict, node, state, &format!("namespace of class {}", c.name.id));
        self.cx.making_class.push((coid, ns, false));
        let strong = self.heap.obj(ns).single;
        let module = self.heap.module_info(self.frame().module).name.clone();
        let str_ = || Value::data(&crate::domain::typed_label("str", ""));
        let qualname = if self.cx.frames.len() <= 1 { Value::lit(key_str_lit(c.name.id.as_str())) } else { str_() };
        let mut items: Vec<(KeyId, Value, bool)> = vec![
            (key_str_lit("__module__"), Value::lit(key_str_lit(&module)), true),
            (key_str_lit("__qualname__"), qualname, true),
            (key_str_lit("__firstlineno__"), Value::lit(intern::key_int(node.line as i64)), true),
        ];
        if matches!(c.body.first(), Some(ast::Stmt::Expr(e)) if e.value.is_string_literal_expr()) {
            items.push((key_str_lit("__doc__"), str_(), true));
        }
        let row = state.row(coid, Ns::Attr);
        let order = row.ordered_keys();
        let body: Vec<KeyId> = order.clone().unwrap_or_else(|| state.slot_keys(coid, Ns::Attr));
        for k in body {
            if row.present.contains(k) && !intern::key_as_str(k).is_some_and(|n| n.starts_with('\u{1}') || NS_ADDED.contains(&&*n)) {
                items.push((k, state.slot(coid, Ns::Attr, k), row.definite.contains(k)));
            }
        }
        let (defs, anns, cell) = crate::syntax::class_namespace_cells(c);
        // (a tuple of strs; cells; the annotations' function)
        if anns {
            items.push((key_str_lit("__annotate_func__"), Value::data("__annotate_func__"), true));
        }
        let statics = self.alloc_positional(Kind::Tuple, &node.sub(1), None, str_(), str_().kv, state);
        items.push((key_str_lit("__static_attributes__"), statics, true));
        for (i, (n, there)) in [("__classdictcell__", defs || anns), ("__classcell__", cell)].into_iter().enumerate() {
            if there {
                let cell = self.alloc_simple(Kind::Cell, &node.sub(2 + i as u32), state, n);
                items.push((key_str_lit(n), Value::obj(cell), true));
            }
        }
        for (k, v, sure) in items {
            self.store_key(ns, Ns::Item, &Value::lit(k), &v, strong && sure, node, state);
        }
        // (the body's names in an order not known: the namespace's too)
        if order.is_none() {
            let r = state.row(ns, Ns::Item).with_order(Vec::new());
            state.set_row(ns, Ns::Item, r);
        }
        ns
    }

    /// Metaclass `m` of the program's makes the class `coid` from namespace `ns` (the running
    /// statement's, `class_namespace`), as Python calls it: `m.__new__(m, name, bases, ns, **kw)`
    /// (`type.__new__` makes the class from what `ns` holds then, `class_from_namespace`),
    /// then, given the class, `m.__init__(cls, name, bases, ns, **kw)`. What `__new__` returns
    /// is what the name is bound to. A `__new__` that did not make the class through
    /// `type.__new__` (or did not run): the class as code rowan does not run left it.
    #[allow(clippy::too_many_arguments)]
    fn run_metaclass(&mut self, c: &'static ast::StmtClassDef, m: ObjId, coid: ObjId, ns: ObjId, base_vals: &[Value], cnode: &Node, state: &mut State) -> Value {
        let kwargs = self.class_keywords(c, state);
        let name = Value::lit(key_str_lit(c.name.id.as_str()));
        let kv = crate::domain::join_values(base_vals.iter().cloned()).kv;
        let bases = self.alloc_positional(Kind::Tuple, &cnode.sub(0x6d62), Some(base_vals.to_vec()), Value::new(), kv, state);
        let args = [name, bases, Value::obj(ns)];
        let skipped = self.budget.skipped;
        let made = match self.class_lookup(m, "__new__") {
            Some(defc) => {
                let pos = [&[Value::obj(m)][..], &args[..]].concat();
                let mut r = Value::new();
                for f in self.heap.class_attr(defc, "__new__").pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect::<Vec<_>>() {
                    r = r.join(&self.call_user(f, None, Args::new(&pos, &kwargs, &[], &[]), state, cnode, false));
                }
                r
            }
            None => {
                self.cx.making_class.last_mut().unwrap().2 = true;
                Value::obj(coid)
            }
        };
        let (_, _, through_type) = self.cx.making_class.pop().unwrap();
        if !through_type {
            self.metaclass_not_run(&Value::obj(m), coid, cnode, state);
        }
        let info = self.heap.class_info(coid);
        info.open_base.set(info.open_base.get() || !through_type);
        if made.pts.contains(coid) || made.pts.is_empty() {
            if let Some(defc) = self.class_lookup(m, "__init__") {
                for f in self.heap.class_attr(defc, "__init__").pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect::<Vec<_>>() {
                    self.call_user(f, Some(Value::obj(coid)), Args::new(&args, &kwargs, &[], &[]), state, cnode, false);
                }
            }
        }
        // (a call that did not run — its own or one it makes — may have done to the class what
        // its code says, as code rowan does not run)
        if through_type && self.budget.skipped > skipped {
            self.metaclass_not_run(&Value::obj(m), coid, cnode, state);
        }
        if made.pts.is_empty() { made.join(&Value::obj(coid)) } else { made }
    }

    /// `type.__new__(mcs, name, bases, ns)` while a class statement's metaclass runs (its
    /// `super().__new__`): that statement's class, its attributes now what `ns` holds (a key
    /// taken out of `ns` is no attribute). None: no class statement is making a class here.
    pub fn class_from_namespace(&mut self, ns: &Value, node: &Node, state: &mut State) -> Option<ObjId> {
        let &(coid, _, false) = self.cx.making_class.last()? else { return None };
        let ns = ns.single().filter(|o| self.heap.kind(*o) == Kind::Dict)?;
        self.cx.making_class.last_mut().unwrap().2 = true;
        let row = state.row(ns, Ns::Item);
        if !row.closed() {
            self.metaclass_not_run(&Value::new(), coid, node, state);
            self.heap.class_info(coid).open_base.set(true);
            return Some(coid);
        }
        let added = |k: KeyId| intern::key_as_str(k).is_some_and(|n| NS_ADDED.contains(&&*n));
        for (k, _) in state.obj_slots(coid, Ns::Attr) {
            if !row.present.contains(k) && !added(k) && !intern::key_as_str(k).is_some_and(|n| n.starts_with('\u{1}')) {
                let r = state.row(coid, Ns::Attr).delete(k, true);
                state.set_row(coid, Ns::Attr, r);
                state.pop_slot(coid, Ns::Attr, k);
                // (no longer the body's: the path decides, `class_lookup_on_path`)
                self.heap.cls_late.insert((coid, k));
            }
        }
        for (k, v) in state.obj_slots(ns, Ns::Item) {
            if added(k) || !row.present.contains(k) || state.slot_opt(coid, Ns::Attr, k).as_ref() == Some(&v) {
                continue;
            }
            let sure = row.definite.contains(k);
            let r = state.row(coid, Ns::Attr).store(k, sure);
            state.set_row(coid, Ns::Attr, r);
            let old = state.slot_opt(coid, Ns::Attr, k);
            state.set_slot(coid, Ns::Attr, k, if sure { v.clone() } else { v.join(&old.unwrap_or_default()) }, true);
            let cls_old = self.heap.cls_slots.get(&(coid, Ns::Attr, k)).cloned().unwrap_or_default();
            self.heap.set_cls_slot((coid, Ns::Attr, k), cls_old.join(&v));
            self.heap.cls_late.insert((coid, k));
        }
        Some(coid)
    }
}
