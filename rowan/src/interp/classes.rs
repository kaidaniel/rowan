//! Class definitions: the class object, its body run in a class frame, its bases and what
//! library bases make of it (Enum, dataclass, pydantic, NamedTuple, TypedDict, nn.Module).

use ruff_python_ast as ast;

use crate::domain::{ObjId, Ns, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s};
use crate::state::{Kind, State};

use super::{Analyzer, Args, ClassInfo, Frame, FrameKind, Payload};

impl Analyzer {
    /// A `class` statement: its bases (library ones say what it is), the class object, its body
    /// run in a class frame (names become class attributes), then its decorators and what its
    /// library bases make of it (enum members, dataclass fields, pydantic, NamedTuple, …).
    pub(super) fn stmt_classdef(&mut self, c: &'static ast::StmtClassDef, state: &mut State) {
        let fr_module = self.frame().module;
        let fr_ctx = self.frame().ctx.clone();
        let fr_single = self.frame().single;
        let fr_cells = self.frame().cells.clone();
        let (bases, mut open_base, lib, ek) = self.class_bases(c, state);
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
        self.run_set_name(c, coid, &cnode, state);
        self.run_init_subclass(c, coid, &info, &cnode, state);
        // (its metaclass's `__new__` / `__init__`, its bases' too, made it: code rowan does not
        // run — it may rebind the data the body defined, or change it; the methods it keeps)
        if let Some(meta) = self.metaclass_of(c, state) {
            let any = Value::obj(super::any_object(&mut self.heap));
            let mut given = vec![meta];
            for (k, v) in state.obj_slots(coid, Ns::Attr) {
                if v.pts.iter().any(|o| self.holds_code(o)) {
                    continue;
                }
                state.set_slot(coid, Ns::Attr, k, v.join(&any), true);
                given.push(v);
            }
            self.unknown_call("metaclass", None, Args::pos(&given), state, &cnode);
        }
        let mut cv = Value::obj(coid);
        if !is_dataclass && !c.decorator_list.is_empty() {
            cv = self.apply_decorators(&c.decorator_list, cnode, cv, state);
            if cv.pts.is_empty() {
                cv = Value::obj(coid);
            }
        }
        self.store_name(s(c.name.id.as_str()), cv, state);
    }

    /// The bases a class statement names: the program's classes, whether the class is open
    /// (a base rowan does not see, a metaclass other than ABCMeta), what its library bases make
    /// of it, and the Enum kind its Enum bases give.
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

    fn class_bases(&mut self, c: &'static ast::StmtClassDef, state: &mut State) -> (Vec<u32>, bool, crate::models::ClassBase, crate::interp::EnumKind) {
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
                    if !abc {
                        open_base = true;
                    }
                }
            }
        }
        (bases, open_base, lib, ek)
    }

    /// The metaclass a class statement, or one of its bases' (the program's classes), names when
    /// it is not ABCMeta; None for none.
    fn metaclass_of(&mut self, c: &'static ast::StmtClassDef, state: &mut State) -> Option<Value> {
        let args = c.arguments.as_deref()?;
        for kw in args.keywords.iter().filter(|k| k.arg.as_ref().is_some_and(|a| a.as_str() == "metaclass")) {
            let mv = self.eval(&kw.value, state);
            let abc = !mv.pts.is_empty() && mv.pts.iter().all(|o| self.heap.kind(o) == Kind::External && self.heap.external_qual(o).ends_with("ABCMeta"));
            if !abc {
                return Some(mv);
            }
        }
        for b in args.args.iter() {
            let bv = self.eval(b, state);
            for o in bv.pts.iter() {
                if let Some(ci) = self.heap.class_info_opt(o).filter(|_| self.heap.kind(o) == Kind::Class) {
                    if let Some(m) = self.metaclass_of(ci.node, state) {
                        return Some(m);
                    }
                }
            }
        }
        None
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
            let mut kwargs: indexmap::IndexMap<intern::StrId, Value> = indexmap::IndexMap::new();
            if let Some(args) = &c.arguments {
                for kw in args.keywords.iter() {
                    if let Some(a) = &kw.arg {
                        if a.as_str() != "metaclass" {
                            let v = self.eval(&kw.value, state);
                            kwargs.insert(s(a.as_str()), v);
                        }
                    }
                }
            }
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
}
