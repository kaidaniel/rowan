//! Annotations: what a value annotated `X` is in a run for any call (a parameter of a general
//! run, a field of an instance such a run makes): a builtin type's unknown (`int(?)`), a
//! container whose parts follow the annotation (`list[str]`), an instance of the program's class
//! (a dataclass's, NamedTuple's, TypedDict's or pydantic model's with its declared fields, each
//! as annotated; another's as its `__init__` makes it), a torch module's tensor, a DataFrame; and
//! what an unknown the program gives a field of a scalar builtin type is (`field_as_annotated`).

use ruff_python_ast as ast;

use super::{Analyzer, FuncInfo, Payload};
use crate::domain::{KeyVal, Ns, ObjId, Row, RowData, Value};
use crate::frontend;
use crate::intern::{self, key_str_lit, KeyId, KeySet};
use crate::source::{Source, SourceKind};
use crate::state::{Kind, State};
use crate::syntax::dotted_last;

impl Analyzer {
    /// The class-bearing part of an annotation: wrappers that keep the value's class are
    /// unwrapped (`Optional`, `Annotated`, `Final`, `ClassVar`, `Required`, a union with `None`
    /// and one other member); a container (`list[Foo]`, `dict[str, Foo]`) is the container
    /// itself, not its element class; a union of several classes has none.
    pub fn strip_annotation(ann: &'static ast::Expr) -> Option<&'static ast::Expr> {
        let is_none = |e: &ast::Expr| matches!(e, ast::Expr::NoneLiteral(_)) || matches!(e, ast::Expr::Name(n) if n.id.as_str() == "None");
        let mut node = ann;
        for _ in 0..16 {
            match node {
                ast::Expr::StringLiteral(sl) => {
                    let txt = sl.value.to_str().to_string();
                    node = frontend::parse_expression(&txt)?;
                }
                ast::Expr::Subscript(sub) => {
                    let elts: Vec<&'static ast::Expr> = match &*sub.slice {
                        ast::Expr::Tuple(t) => t.elts.iter().collect(),
                        e => vec![e],
                    };
                    match dotted_last(&sub.value).as_str() {
                        "Optional" | "Annotated" | "Final" | "ClassVar" | "Required" | "NotRequired" | "ReadOnly" => node = elts.first()?,
                        "Union" => {
                            let rest: Vec<&'static ast::Expr> = elts.into_iter().filter(|e| !is_none(e)).collect();
                            if rest.len() != 1 {
                                return None;
                            }
                            node = rest.first()?;
                        }
                        // (a container: its own class; the old rule took the element's)
                        _ => return Some(&sub.value),
                    }
                }
                ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr => {
                    // (`A | B | None`: flattened)
                    let mut parts: Vec<&'static ast::Expr> = Vec::new();
                    let mut stack: Vec<&'static ast::Expr> = vec![node];
                    while let Some(e) = stack.pop() {
                        match e {
                            ast::Expr::BinOp(b2) if b2.op == ast::Operator::BitOr => {
                                stack.push(&b2.right);
                                stack.push(&b2.left);
                            }
                            e => parts.push(e),
                        }
                    }
                    let rest: Vec<&'static ast::Expr> = parts.into_iter().filter(|e| !is_none(e)).collect();
                    if rest.len() != 1 {
                        return None;
                    }
                    node = rest.first()?;
                }
                _ => return Some(node),
            }
        }
        None
    }

    /// Unknown `o`, what a caller passes for a parameter annotated `ann` (written in `module`),
    /// as the annotation says when it names a builtin type: of that type (`int(?)`; a
    /// container's parts too), with None beside it when it allows None.
    pub fn as_annotated(&mut self, o: ObjId, ann: &'static ast::Expr, module: ObjId) -> Value {
        let Some((t, none)) = builtin_annotation(ann) else { return Value::obj(o) };
        self.heap.obj_mut(o).ty = Some(t);
        if let Some(c) = container_annotation(ann) {
            self.heap.ann.insert(o, (c, module));
        }
        if none { Value::obj(o).join(&Value::none()) } else { Value::obj(o) }
    }

    /// The value a parameter annotated `ann` takes in a run for any call: the torch model's
    /// tensor or module, an instance of a class of the program (`annotated_instance`), an open
    /// DataFrame; None when the annotation names none of those.
    pub(in crate::interp) fn param_from_annotation(&mut self, ann: &'static ast::Expr, fi: &FuncInfo, pname: &str, state: &mut State) -> Option<Value> {
        let node = Self::strip_annotation(ann)?;
        if let Some(v) = crate::models::torch::t_param(self, ann, fi, pname, state) {
            return Some(v);
        }
        let mut v = self.eval(node, state);
        // (a class imported only for type checkers)
        if let ast::Expr::Name(n) = node {
            if !v.pts.iter().any(|o| self.heap.kind(o) == Kind::Class) {
                v = self.type_only_name(n.id.as_str(), state).unwrap_or(v);
            }
        }
        for oid in v.pts.iter() {
            let o = self.heap.kind(oid);
            if o == Kind::Class {
                let inst = self.annotated_instance(oid, &format!("{}.{}", fi.qualname, pname), Some((fi, pname)), true, 0, state);
                // (`Optional[C]`, `C | None`: or None)
                return match annotation_members(ann).1 {
                    true => Some(inst.unwrap_or_default().join(&Value::none())),
                    false => inst,
                };
            }
            if o == Kind::External && self.heap.external_qual(oid).ends_with("DataFrame") {
                let src = Source::new(&format!("param {}.{}", fi.py_qualname(), pname), SourceKind::Param);
                let fid = self.alloc(
                    Kind::Frame,
                    &format!("param:{}.{}", fi.qualname, pname),
                    state,
                    "frame param",
                    Payload::None,
                    Some(Row::with_source(src)),
                    Ns::Item,
                    Some(true),
                    Some(&format!("entry:{}", fi.qualname)),
                );
                return Some(Value::obj(fid));
            }
        }
        None
    }

    /// An instance of the program's class `cls` standing for any instance an annotation allows
    /// (`path`: what it is, `F.p`, `F.p.field`, `F.p.field[*]`; `param`: the parameter it is, at
    /// the top; `single`: it stands for one runtime object; `depth`: how many parts below a
    /// parameter it is). A class with declared fields (`declared_fields`) has them and nothing
    /// else, each holding what its annotation allows (`field_value`), made once per path; another
    /// is made as its `__init__` makes it (None: no path of it completes).
    fn annotated_instance(&mut self, cls: ObjId, path: &str, param: Option<(&FuncInfo, &str)>, single: bool, depth: u32, state: &mut State) -> Option<Value> {
        // (an instance of a subclass passes for one: those of the program's subclasses that
        // override something of the class (a method, `__init__`, a class attribute), each made
        // as the class's is, up to `SUBCLASS_INSTANCES`; past them an unknown instead, whose
        // methods are unknown calls: a base of many would make them all at every parameter)
        const SUBCLASS_INSTANCES: usize = 2;
        let overrides = |an: &Analyzer, c: ObjId| {
            an.heap.mro(c).into_iter().take_while(|m| *m != cls).any(|m| {
                an.heap.class_info_opt(m).is_some_and(|ci| ci.node.body.iter().any(|st| defined_names(st).iter().any(|n| an.class_lookup(cls, n).is_some())))
            })
        };
        let subs: Vec<ObjId> = self.program.user_classes.iter().copied().filter(|c| *c != cls && self.heap.mro(*c).contains(&cls) && overrides(self, *c)).collect();
        let mut out = self.class_instance(cls, path, param, single, depth, state);
        // (an instance of a subclass that overrides nothing is the class's but for its class:
        // the class's instance stands for those too, `Obj::any_sub`)
        if self.program.user_classes.iter().any(|c| *c != cls && !subs.contains(c) && self.heap.mro(*c).contains(&cls)) {
            for o in out.iter().flat_map(|v| v.pts.iter()).filter(|o| self.heap.instance_class(*o) == Some(cls)).collect::<Vec<_>>() {
                self.heap.obj_mut(o).any_sub = true;
            }
        }
        if subs.len() > SUBCLASS_INSTANCES {
            return Some(out.unwrap_or_default().join(&Value::data(&format!("instance {}", path))));
        }
        for c in subs {
            if let Some(v) = self.class_instance(c, path, param, single, depth, state) {
                out = Some(out.unwrap_or_default().join(&v));
            }
        }
        out
    }

    /// The classes instance `o` may be of: its class, and, for one standing for its subclasses'
    /// instances too (`Obj::any_sub`), the program's subclasses (None past `ANY_SUB_CLASSES` of
    /// them: not known).
    pub fn classes_of_instance(&self, o: ObjId) -> Option<Vec<ObjId>> {
        const ANY_SUB_CLASSES: usize = 8;
        let cls = self.heap.instance_class(o)?;
        if !self.heap.obj(o).any_sub {
            return Some(vec![cls]);
        }
        let mut out = vec![cls];
        out.extend(self.program.user_classes.iter().copied().filter(|c| *c != cls && self.heap.mro(*c).contains(&cls)));
        (out.len() <= ANY_SUB_CLASSES).then_some(out)
    }

    /// `annotated_instance` of `cls` itself.
    fn class_instance(&mut self, cls: ObjId, path: &str, param: Option<(&FuncInfo, &str)>, single: bool, depth: u32, state: &mut State) -> Option<Value> {
        let ci = self.heap.class_info(cls);
        let Some((fields, extra, validated)) = self.declared_fields(cls, state) else {
            // (a part's, `list[B]`'s element: made once per path, as one with declared fields —
            // made again, the earlier one still in use, it would be no single object, its
            // `__init__`'s stores weak)
            let ctx = format!("entry:{}", path);
            if let Some(o) = self.made_instance(cls, &ctx, state).filter(|_| param.is_none()) {
                return Some(Value::obj(o));
            }
            // (made where a path first reads the part: what it made starts so on every path)
            let pre = state.copy();
            let inst = self.fresh_instance(cls, state, &ctx);
            for (o, ns) in state.rows.diff_keys(&pre.rows) {
                if pre.has_facts(o) {
                    continue;
                }
                let Some(r) = state.rows.get(&(o, ns)) else { continue };
                crate::state::mark_found(o, ns, &r);
            }
            for (o, sk) in state.slots.diff_keys(&pre.slots) {
                if let (false, Some(v)) = (pre.has_facts(o), state.slots.get(&o, &sk)) {
                    crate::state::mark_found_slot(o, sk, &v);
                }
            }
            return inst.map(Value::obj);
        };
        let ns = if ci.lib.typed_dict { Ns::Item } else { Ns::Attr };
        let mut names: KeySet = KeySet::from_iter(fields.iter().map(|(f, _)| key_str_lit(f)));
        let mut declared = Row::closed_with(names.clone());
        if extra {
            // (`extra="allow"`: other attributes may be there)
            declared = Row::with_source(Source::new(&format!("instance {}", ci.name), SourceKind::Class));
            for f in names.iter() {
                declared = declared.store(f, true);
            }
        }
        // a TypedDict's `total=False` / `NotRequired` keys may be absent
        if ci.lib.typed_dict {
            if let Some((req, opt)) = crate::models::dataclasses::typed_dict_keys(self, cls) {
                names = req.union(&opt);
                declared = Row::from_data(RowData { present: names.clone(), definite: req.clone(), likely: req, ..Default::default() });
            }
        }
        let label = format!("{} param", ci.name);
        // (a TypedDict's value is a plain dict: dict's methods, its keys as declared)
        let kind = if ci.lib.typed_dict { Kind::Dict } else { Kind::Instance };
        let inst = match param {
            Some((fi, _)) => self.alloc(kind, &format!("param:{}:{}#{}", path, ci.name, cls), state, &label, Payload::Cls(cls), Some(declared), ns, Some(true), Some(&format!("entry:{}", fi.qualname))),
            None => {
                let o = self.heap.alloc_raw(kind, &format!("ann:{}#{}", path, cls), "", single, &label, Payload::Cls(cls));
                if state.has_row(o, ns) {
                    return Some(Value::obj(o));
                }
                self.heap.obj_mut(o).depth = depth;
                crate::state::mark_found(o, ns, &declared);
                state.set_row(o, ns, declared);
                o
            }
        };
        for f in names.iter() {
            let name = intern::key_py_str(f);
            let ann = fields.iter().find(|(n, _)| *n == name).and_then(|(_, a)| *a);
            let v = match ann {
                Some((a, module)) => self.field_value(a, module, validated, &format!("{}.{}", path, name), single, depth, state),
                None => Value::data(&format!("field {}", name)),
            };
            if param.is_none() {
                crate::state::mark_found_slot(inst, (ns, f), &v);
            }
            state.set_slot(inst, ns, f, v, true);
        }
        if let (true, Some((fi, pname))) = (ci.lib.typed_dict, param) {
            let src = Source::new(&format!("param {}.{}", fi.py_qualname(), pname), SourceKind::Param);
            self.m.td_param_src.insert(inst, src);
        }
        Some(Value::obj(inst))
    }

    /// `v`, given by the program for field `name` of `cls` (its generated `__init__`), as the
    /// field's annotation says when that names one scalar builtin type (`str`, `Optional[int]`;
    /// `float` only when `validated`: an int passes for one otherwise): its unknown parts
    /// (untyped unknown scalars, unknown objects of no known type) are of that type, or None
    /// when the annotation allows it; literals and the program's objects stay as they are.
    pub fn field_as_annotated(&mut self, cls: ObjId, name: &str, v: Value, validated: bool, state: &mut State) -> Value {
        let Some((ann, _)) = self.field_annotation(cls, name) else { return v };
        let (members, none) = annotation_members(ann);
        let [m] = members[..] else { return v };
        let t = match builtin_annotation(m) {
            Some((t, _)) if matches!(t, "bool" | "bytes" | "int" | "str") => t,
            _ if validated && dotted_last(m) == "float" && !matches!(m, ast::Expr::Subscript(_)) => "float",
            _ => return v,
        };
        // (an unknown carrying a model's facts — a tensor dim, a shape — keeps them)
        let special = |an: &Analyzer, o: ObjId| an.m.t_numbers.contains_key(&o);
        let unknown_objs: Vec<ObjId> = v.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Unknown && self.heap.obj(*o).ty.is_none() && !special(self, *o)).collect();
        let scalar = match &v.kv {
            KeyVal::Data(l, _) => {
                let l = intern::str_get(*l);
                (crate::domain::label_type(&l).is_none() && !l.starts_with('\u{1}')).then(|| crate::domain::label_name(&l).to_string())
            }
            _ => None,
        };
        if unknown_objs.is_empty() && scalar.is_none() {
            return v;
        }
        let mut out = Value::objs(crate::domain::ObjSet::from_vec(v.pts.iter().filter(|o| !unknown_objs.contains(o)).collect()));
        out.kv = if scalar.is_some() { KeyVal::Bot } else { v.kv.clone() };
        // (beside None: an unknown object of the type, as a general run's field holds)
        let typed = match none {
            false => Value::data(&crate::domain::typed_label(t, scalar.as_deref().unwrap_or(""))),
            true => self.typed_object(&format!("ann:{}#{}.{}", self.heap.class_info(cls).name, cls, name), t, false, 0, None, state).join(&Value::none()),
        };
        out.join(&typed)
    }

    /// An unknown object of builtin type `t` made at `site` (one beside other values, or a
    /// container whose parts follow annotation `ann`).
    fn typed_object(&mut self, site: &str, t: &'static str, single: bool, depth: u32, ann: Option<(&'static ast::Expr, ObjId)>, state: &mut State) -> Value {
        let o = self.heap.alloc_raw(Kind::Unknown, site, "", single, "?", Payload::None);
        crate::state::mark_found(o, Ns::Item, &Row::top());
        crate::state::mark_found(o, Ns::Attr, &Row::top());
        self.heap.obj_mut(o).ty = Some(t);
        self.heap.obj_mut(o).depth = depth;
        if let Some(a) = ann {
            self.heap.ann.insert(o, a);
        }
        for n in [Ns::Item, Ns::Attr] {
            if !state.has_row(o, n) {
                state.set_row(o, n, Row::top());
            }
        }
        Value::obj(o)
    }

    /// What part `k` of unknown `oid` holds when `oid` follows a container annotation whose part
    /// there names a class of the program, or a union with one (`list[User]`, `dict[str,
    /// Optional[Item]]`): an instance of it (`annotated_instance`; the element standing for
    /// every element: not one runtime object), None, …; None when the part is of a builtin
    /// type, nothing, or too deep (the placeholder of an unknown's part stands for it).
    pub(in crate::interp) fn part_instance(&mut self, oid: ObjId, k: KeyId, state: &mut State) -> Option<Value> {
        let &(ann, module) = self.heap.ann.get(&oid)?;
        let part = part_annotation(ann, k)?;
        let depth = self.heap.obj(oid).depth;
        if builtin_annotation(part).is_some() || depth + 1 >= super::PATH_DEPTH {
            return None;
        }
        let path = format!("{}[{}]", oid, intern::key_repr(k));
        let v = self.field_value(part, module, false, &path, k != intern::STAR, depth, state);
        // (a class the annotation does not name in the program: an unknown, the placeholder)
        let any = v.pts.iter().any(|o| self.heap.kind(o) == Kind::Instance);
        any.then_some(v)
    }

    /// What iterating unknown `oid` gives when it follows a dict annotation (`dict[str, V]`):
    /// a key, of the annotation's key type; None for anything else (an element, `oid[*]`).
    pub(in crate::interp) fn annotated_key(&mut self, oid: ObjId, state: &mut State) -> Option<Value> {
        let &(ann, module) = self.heap.ann.get(&oid)?;
        let ast::Expr::Subscript(sub) = ann else { return None };
        let ast::Expr::Tuple(t) = &*sub.slice else { return None };
        let (Some(("dict", _)), [kt, _]) = (builtin_annotation(ann), t.elts.as_slice()) else { return None };
        let depth = self.heap.obj(oid).depth;
        Some(self.field_value(kt, module, false, &format!("{}.keys", oid), false, depth, state))
    }

    /// The fields of a class whose instances hold its declared fields and nothing else: a
    /// dataclass's, NamedTuple's or TypedDict's (its bases' too), a pydantic model's (not a root
    /// model's); each with its annotation and the module it is written in (none for a field
    /// whose validator may return anything); whether other attributes may be there too (a
    /// model's `extra="allow"`); whether values are validated (an int given for a `float`
    /// becomes a float).
    #[allow(clippy::type_complexity)]
    fn declared_fields(&mut self, cls: ObjId, state: &State) -> Option<(Vec<(String, Option<(&'static ast::Expr, ObjId)>)>, bool, bool)> {
        if crate::models::pydantic::is_model(self, cls) {
            let pc = crate::models::pydantic::pyd_class(self, cls, state);
            if pc.root {
                return None;
            }
            // (a validator after or instead of pydantic's may return anything)
            let free = |f: &str| pc.field_validators.iter().any(|v| v.mode != "before" && v.fields.iter().any(|x| x == f || x == "*"));
            let fields = pc.fields.iter().map(|f| (f.name.clone(), if free(&f.name) { None } else { self.field_annotation(cls, &f.name) })).collect();
            return Some((fields, pc.extra == crate::models::pydantic::Extra::Allow, true));
        }
        if !self.heap.class_info(cls).has_fields() {
            return None;
        }
        let validated = crate::models::pydantic::is_validated(self, cls);
        let fields = crate::models::dataclasses::dataclass_fields_mro(self, cls).into_iter().map(|(f, _)| {
            let a = self.field_annotation(cls, &f);
            (f, a)
        });
        Some((fields.collect(), false, validated))
    }

    /// The annotation of field `name` of `cls` (the nearest class of its MRO declaring it) and
    /// the module it is written in.
    fn field_annotation(&self, cls: ObjId, name: &str) -> Option<(&'static ast::Expr, ObjId)> {
        self.heap.mro(cls).into_iter().find_map(|c| {
            let ci = self.heap.class_info_opt(c)?;
            ci.node.body.iter().find_map(|st| match st {
                ast::Stmt::AnnAssign(a) if matches!(&*a.target, ast::Expr::Name(n) if n.id.as_str() == name) => Some((&*a.annotation, ci.module)),
                _ => None,
            })
        })
    }

    /// What a field annotated `ann` (written in `module`) holds on any instance: each member of
    /// a union (`Optional[X]`, `X | None`) as it allows: None; a `Literal`'s values; a builtin
    /// type's unknown (`str(?)`; `float` only when `validated`: an int passes for a float
    /// otherwise); a builtin container whose parts follow the annotation; an Enum's members; an
    /// instance of the program's class (`annotated_instance`, up to `MAX_NEST` deep); else
    /// unknown.
    #[allow(clippy::too_many_arguments)]
    fn field_value(&mut self, ann: &'static ast::Expr, module: ObjId, validated: bool, path: &str, single: bool, depth: u32, state: &mut State) -> Value {
        const MAX_NEST: u32 = 2;
        let unknown = || Value::data(&format!("field {}", path.rsplit('.').next().unwrap_or(path)));
        let (members, none) = annotation_members(ann);
        let alone = members.len() == 1 && !none;
        // an unknown of builtin type `t`: a scalar alone is a typed unknown (`str(?)`); one of
        // several members, or a container (its parts as annotated `c` says), an unknown object
        // of that type
        let typed = |an: &mut Analyzer, t: &'static str, c: Option<&'static ast::Expr>, k: usize, state: &mut State| {
            if alone && c.is_none() && crate::domain::TYPE_NAMES.contains(&t) {
                return Value::data(&crate::domain::typed_label(t, ""));
            }
            an.typed_object(&format!("ann:{}:{}", path, k), t, single, depth + 1, c.map(|c| (c, module)), state)
        };
        let mut out = if none { Value::none() } else { Value::new() };
        if members.is_empty() && !none {
            return unknown();
        }
        for (k, m) in members.into_iter().enumerate() {
            let v = match m {
                ast::Expr::Subscript(sub) if dotted_last(&sub.value) == "Literal" => {
                    let elts: Vec<&'static ast::Expr> = match &*sub.slice {
                        ast::Expr::Tuple(t) => t.elts.iter().collect(),
                        e => vec![e],
                    };
                    let vals: Vec<Value> = elts.into_iter().map(|e| self.in_module(module, "ann", "<annotation>", |an| an.eval(e, state))).collect();
                    if vals.iter().all(|v| v.pts.is_empty() && v.kv.is_literal()) {
                        vals.into_iter().fold(Value::new(), |a, v| a.join(&v))
                    } else {
                        unknown()
                    }
                }
                _ if validated && dotted_last(m) == "float" && !matches!(m, ast::Expr::Subscript(_)) => typed(self, "float", None, k, state),
                _ => match builtin_annotation(m) {
                    Some((t, _)) => typed(self, t, container_annotation(m), k, state),
                    None => {
                        let cv = self.in_module(module, "ann", "<annotation>", |an| {
                            let v = an.eval(m, state);
                            match m {
                                ast::Expr::Name(n) if !v.pts.iter().any(|o| an.heap.kind(o) == Kind::Class) => an.type_only_name(n.id.as_str(), state).unwrap_or(v),
                                _ => v,
                            }
                        });
                        match cv.single().filter(|c| self.heap.kind(*c) == Kind::Class && self.heap.class_info_opt(*c).is_some()) {
                            Some(c) if self.m.enum_members.contains_key(&c) => Value::objs(crate::domain::ObjSet::from_vec(self.m.enum_members[&c].iter().map(|(_, o)| *o).collect())),
                            Some(c) if depth < MAX_NEST => self.annotated_instance(c, path, None, single, depth + 1, state).unwrap_or_else(unknown),
                            _ => unknown(),
                        }
                    }
                },
            };
            out = out.join(&v);
        }
        out
    }
}

/// The builtin type an annotation names (`int`, `"str"`, `list[int]`, `Optional[str]`, `int |
/// None`), and whether it allows None: a type whose instances a run for any call may take its
/// parameter to be (`float` is left out: an int may be given for it).
pub(in crate::interp) fn builtin_annotation(ann: &ast::Expr) -> Option<(&'static str, bool)> {
    const TYPES: [(&str, &str); 14] = [("bool", "bool"), ("bytes", "bytes"), ("int", "int"), ("str", "str"), ("list", "list"), ("dict", "dict"), ("set", "set"), ("tuple", "tuple"), ("frozenset", "frozenset"), ("List", "list"), ("Dict", "dict"), ("Set", "set"), ("Tuple", "tuple"), ("FrozenSet", "frozenset")];
    let is_none = |e: &ast::Expr| matches!(e, ast::Expr::NoneLiteral(_)) || matches!(e, ast::Expr::Name(n) if n.id.as_str() == "None");
    let named = |e: &ast::Expr| TYPES.iter().find(|(n, _)| *n == dotted_last(e)).map(|(_, t)| *t);
    match ann {
        ast::Expr::Name(_) | ast::Expr::Attribute(_) => named(ann).map(|t| (t, false)),
        ast::Expr::StringLiteral(sl) => frontend::parse_expression(sl.value.to_str()).and_then(builtin_annotation),
        // (`list[int]`: a list; `Optional[str]`: a str or None)
        ast::Expr::Subscript(sub) if dotted_last(&sub.value) == "Optional" => builtin_annotation(&sub.slice).map(|(t, _)| (t, true)),
        ast::Expr::Subscript(sub) => named(&sub.value).filter(|t| !matches!(*t, "bool" | "bytes" | "int" | "str")).map(|t| (t, false)),
        ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr && (is_none(&b.left) || is_none(&b.right)) => {
            builtin_annotation(if is_none(&b.right) { &b.left } else { &b.right }).map(|(t, _)| (t, true))
        }
        _ => None,
    }
}

/// The annotation of a builtin container a value follows (`list[dict[str, int]]`; quotes,
/// `Optional` and `| None` stripped); None for a bare type or anything else.
pub(in crate::interp) fn container_annotation(ann: &'static ast::Expr) -> Option<&'static ast::Expr> {
    let is_none = |e: &ast::Expr| matches!(e, ast::Expr::NoneLiteral(_)) || matches!(e, ast::Expr::Name(n) if n.id.as_str() == "None");
    match ann {
        ast::Expr::StringLiteral(sl) => frontend::parse_expression(sl.value.to_str()).and_then(container_annotation),
        ast::Expr::Subscript(sub) if dotted_last(&sub.value) == "Optional" => container_annotation(&sub.slice),
        ast::Expr::Subscript(_) => builtin_annotation(ann).map(|_| ann),
        ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr && (is_none(&b.left) || is_none(&b.right)) => container_annotation(if is_none(&b.right) { &b.left } else { &b.right }),
        _ => None,
    }
}

/// The annotation of a part of a value annotated `ann` (`container_annotation`): of a list's or
/// set's element, a dict's value, a tuple's position `k`.
pub(in crate::interp) fn part_annotation(ann: &'static ast::Expr, k: KeyId) -> Option<&'static ast::Expr> {
    let ast::Expr::Subscript(sub) = ann else { return None };
    let args: Vec<&'static ast::Expr> = match &*sub.slice {
        ast::Expr::Tuple(t) => t.elts.iter().collect(),
        e => vec![e],
    };
    match (builtin_annotation(ann)?.0, args.as_slice()) {
        ("list" | "set" | "frozenset", [e]) => Some(e),
        // (any key's value; its keys, what iterating it gives, are `annotated_key`)
        ("dict", [_, vt]) => Some(vt),
        ("tuple", [e, ast::Expr::EllipsisLiteral(_)]) => Some(e),
        ("tuple", ts) => intern::key_as_int(k).and_then(|i| ts.get(usize::try_from(i).ok()?).copied()),
        _ => None,
    }
}

/// For people: the type an annotation gives, unknowns of builtin types marked, classes by name
/// (`list[dict[str(?), int(?)]]`, `list[User]`; anything else `?`); a part that may be None
/// says so, the whole does not (the value it is shown for has the None).
pub(in crate::interp) fn annotation_text(ann: &'static ast::Expr) -> String {
    fn text(ann: &'static ast::Expr, top: bool) -> String {
        let Some((t, none)) = builtin_annotation(ann) else {
            // (a class by its name, `list[User]`)
            return match ann {
                ast::Expr::Name(n) if n.id.as_str() != "Any" => n.id.to_string(),
                ast::Expr::StringLiteral(sl) => frontend::parse_expression(sl.value.to_str()).map(|e| text(e, top)).unwrap_or_else(|| "?".into()),
                ast::Expr::Subscript(sub) if dotted_last(&sub.value) == "Optional" && !top => format!("{} | None", text(&sub.slice, true)),
                _ => "?".into(),
            };
        };
        let s = match container_annotation(ann) {
            Some(ast::Expr::Subscript(sub)) => {
                let args: Vec<String> = match &*sub.slice {
                    ast::Expr::Tuple(tu) => tu.elts.iter().map(|e| if e.is_ellipsis_literal_expr() { "...".into() } else { text(e, false) }).collect(),
                    e => vec![text(e, false)],
                };
                format!("{}[{}]", t, args.join(", "))
            }
            _ => format!("{}(?)", t),
        };
        if none && !top { format!("{} | None", s) } else { s }
    }
    text(ann, true)
}

/// The members of a union an annotation is (`A | B`, `Union[A, B]`, `Optional[X]`), with what
/// keeps a value's type unwrapped (quotes, `Annotated`, `Final`, `Required`, …), and whether
/// None is one of them.
fn annotation_members(ann: &'static ast::Expr) -> (Vec<&'static ast::Expr>, bool) {
    let mut out = Vec::new();
    let mut none = false;
    let mut work = vec![ann];
    while let Some(e) = work.pop() {
        if out.len() > 16 {
            break;
        }
        match e {
            ast::Expr::StringLiteral(sl) => match frontend::parse_expression(sl.value.to_str()) {
                Some(x) => work.push(x),
                None => out.push(e),
            },
            ast::Expr::Subscript(sub) => {
                let elts: Vec<&'static ast::Expr> = match &*sub.slice {
                    ast::Expr::Tuple(t) => t.elts.iter().collect(),
                    x => vec![x],
                };
                match dotted_last(&sub.value).as_str() {
                    "Optional" => {
                        none = true;
                        work.extend(elts);
                    }
                    "Union" => work.extend(elts.into_iter().rev()),
                    "Annotated" | "Final" | "Required" | "NotRequired" | "ReadOnly" => work.extend(elts.first().copied()),
                    _ => out.push(e),
                }
            }
            ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr => {
                work.push(&b.right);
                work.push(&b.left);
            }
            ast::Expr::NoneLiteral(_) => none = true,
            ast::Expr::Name(n) if n.id.as_str() == "None" => none = true,
            _ => out.push(e),
        }
    }
    (out, none)
}

/// The names a statement of a class body binds: a `def`'s, an assignment's targets.
fn defined_names(st: &ast::Stmt) -> Vec<&str> {
    match st {
        ast::Stmt::FunctionDef(f) => vec![f.name.as_str()],
        ast::Stmt::Assign(a) => a.targets.iter().filter_map(|t| if let ast::Expr::Name(n) = t { Some(n.id.as_str()) } else { None }).collect(),
        ast::Stmt::AnnAssign(a) if a.value.is_some() => match &*a.target {
            ast::Expr::Name(n) => vec![n.id.as_str()],
            _ => vec![],
        },
        _ => vec![],
    }
}
