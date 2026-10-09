//! pydantic (SPECIFICATION §11.2): a `BaseModel` subclass is a class with declared fields.
//! Construction (`M(**kw)`, `M.model_validate(d)`, `parse_obj`, `TypeAdapter`) reads each
//! field from the input by its alias / name: a required field is a read (a missing one fails,
//! an open input gets a requirement), a field with a default is a guarded read; values are
//! validated by the annotation (a nested model from a dict, a list of models from a list).
//! Validators are user code run with what pydantic passes them (`mode="before"`: the raw
//! input). The instance's attributes are the fields (closed unless `extra="allow"`);
//! `model_dump` / `dict` / `model_dump_json` give the fields as keys.

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, Classification, KeyVal, Ns, ObjId, Row, Tier, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::interp::{dotted_last, Analyzer, Call, Payload};
use crate::state::{Kind, State};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Extra {
    Ignore,
    Allow,
    Forbid,
}

/// A field's declared type, as far as validation changes the value.
#[derive(Clone, Debug)]
pub enum Ann {
    Model(ObjId),
    List(Box<Ann>),
    Dict(Box<Ann>),
    Optional(Box<Ann>),
    Other,
}

#[derive(Clone, Debug)]
pub struct PField {
    pub name: String,
    /// the key the input uses (`alias` / `validation_alias`)
    pub alias: Option<String>,
    /// the key `model_dump(by_alias=True)` uses (`serialization_alias`, else `alias`)
    pub ser_alias: Option<String>,
    /// further input keys (`AliasChoices("ts", "timestamp")`: `timestamp`)
    pub alts: Vec<String>,
    pub default: Option<&'static ast::Expr>,
    pub factory: Option<&'static ast::Expr>,
    pub required: bool,
    /// `Optional[X]` / `X | None`: the value may be None
    pub optional: bool,
    pub ann: Ann,
}

#[derive(Clone, Debug)]
pub struct Validator {
    pub func: ObjId,
    /// the fields (`*`: all) of a field validator; empty for a model validator
    pub fields: Vec<String>,
    /// "before" / "after" / "wrap" / "plain"
    pub mode: String,
}

#[derive(Clone, Debug)]
pub struct PClass {
    pub fields: Vec<PField>,
    pub extra: Extra,
    pub populate_by_name: bool,
    pub root: bool,
    pub model_before: Vec<Validator>,
    pub model_after: Vec<Validator>,
    /// v1 `@root_validator` (post): gets the dict of field values, returns it
    pub root_post: Vec<Validator>,
    pub field_validators: Vec<Validator>,
    pub computed: Vec<String>,
    /// private attributes (`_x: T = PrivateAttr(default=…)` / `= value`): name, default, factory
    pub privates: Vec<(String, Option<&'static ast::Expr>, Option<&'static ast::Expr>)>,
}

/// The slot key of private attribute `name` (kept out of the attribute row: not in `vars()`).
pub fn priv_key(name: &str) -> KeyId {
    key_str_lit(&format!("\u{2}{}", name))
}

/// pydantic's methods and attributes on models and model classes.
pub const PYD_METHODS: &[&str] = &[
    "model_dump", "model_dump_json", "model_copy", "model_fields", "model_fields_set", "model_computed_fields", "model_json_schema", "model_validate",
    "model_validate_json", "model_validate_strings", "model_construct", "model_extra", "model_post_init", "dict", "json", "copy", "parse_obj", "parse_raw",
    "construct", "schema", "__fields__", "__fields_set__", "validate",
];

/// pydantic's members that are attributes (properties), not methods.
pub const PYD_ATTRS: &[&str] = &["model_fields", "model_fields_set", "model_computed_fields", "model_extra", "__fields__", "__fields_set__"];

/// The value of attribute `name` (one of `PYD_ATTRS`) on a model instance / class.
pub fn attr_value(an: &mut Analyzer, st: &mut State, recv: ObjId, name: &str, node: &Node) -> Value {
    let c = Call { qual: format!("pydantic.{}", name), recv: Some(Value::obj(recv)), args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
    if an.kind(recv) == Kind::Class {
        m_class(an, st, &c)
    } else {
        m_instance(an, st, &c)
    }
}

/// Is `cls` a pydantic model class (a `BaseModel` / `BaseSettings` / `RootModel` above it)?
pub fn is_model(an: &Analyzer, cls: ObjId) -> bool {
    an.class_info_opt(cls).is_some() && an.mro(cls).iter().any(|c| an.class_info_opt(*c).map(|x| x.pydantic).unwrap_or(false))
}

/// Is `oid` an instance of a pydantic model class?
pub fn is_instance(an: &Analyzer, oid: ObjId) -> bool {
    an.kind(oid) == Kind::Instance && an.instance_class(oid).map(|c| is_model(an, c)).unwrap_or(false)
}

/// The base expression names a pydantic base class (`BaseModel`, `BaseSettings`, `RootModel`,
/// `pydantic.BaseModel`, generic `RootModel[list[int]]`).
pub fn is_pydantic_base(q: &str) -> bool {
    (q.starts_with("pydantic") || q.starts_with("pydantic_settings")) && (q.ends_with("BaseModel") || q.ends_with("BaseSettings") || q.ends_with("RootModel") || q.ends_with("GenericModel"))
}

fn kw<'a>(c: &'a ast::ExprCall, name: &str) -> Option<&'a ast::Expr> {
    c.arguments.keywords.iter().find(|k| k.arg.as_ref().map(|a| a.as_str() == name).unwrap_or(false)).map(|k| &k.value)
}

fn str_lit(e: &ast::Expr) -> Option<String> {
    match e {
        ast::Expr::StringLiteral(s0) => Some(s0.value.to_str().to_string()),
        _ => None,
    }
}

fn bool_lit(e: &ast::Expr) -> Option<bool> {
    match e {
        ast::Expr::BooleanLiteral(b) => Some(b.value),
        _ => None,
    }
}

fn is_ellipsis(e: &ast::Expr) -> bool {
    matches!(e, ast::Expr::EllipsisLiteral(_))
}

/// The leaf name of a type expression (`typing.Optional` → `Optional`).
fn type_name(e: &ast::Expr) -> String {
    match e {
        ast::Expr::Name(n) => n.id.to_string(),
        ast::Expr::Attribute(a) => a.attr.id.to_string(),
        ast::Expr::StringLiteral(s0) => s0.value.to_str().trim().to_string(),
        _ => String::new(),
    }
}

impl Analyzer {
    /// The pydantic description of model class `cls` (cached).
    pub fn pyd_class(&mut self, cls: ObjId, state: &State) -> Rc<PClass> {
        if let Some(p) = self.pyd_classes.get(&cls) {
            return p.clone();
        }
        let mut fields: Vec<PField> = Vec::new();
        let mut extra = Extra::Ignore;
        let mut populate_by_name = false;
        let mut root = false;
        let (mut model_before, mut model_after, mut root_post, mut field_validators, mut computed) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut privates: Vec<(String, Option<&'static ast::Expr>, Option<&'static ast::Expr>)> = Vec::new();
        // bases first: a subclass overrides fields, config and validators keep accumulating
        for c in self.mro(cls).into_iter().rev() {
            let Some(ci) = self.class_info_opt(c) else { continue };
            if !is_model(self, c) {
                continue;
            }
            let cnode: &'static ast::StmtClassDef = ci.node;
            if let Some(args) = &cnode.arguments {
                for b in args.args.iter() {
                    if let ast::Expr::Subscript(sb) = b {
                        if type_name(&sb.value) == "RootModel" {
                            root = true;
                        }
                    } else if type_name(b) == "RootModel" {
                        root = true;
                    }
                }
                // `class M(BaseModel, extra="allow")`
                for k in args.keywords.iter() {
                    match k.arg.as_ref().map(|a| a.as_str()) {
                        Some("extra") => extra = extra_of(&k.value).unwrap_or(extra),
                        Some("populate_by_name") => populate_by_name = bool_lit(&k.value).unwrap_or(populate_by_name),
                        _ => {}
                    }
                }
            }
            for st in &cnode.body {
                match st {
                    ast::Stmt::AnnAssign(a) => {
                        let ast::Expr::Name(n) = &*a.target else { continue };
                        let name = n.id.to_string();
                        if name == "model_config" {
                            if let Some(v) = &a.value {
                                config_of(v, &mut extra, &mut populate_by_name);
                            }
                            continue;
                        }
                        if name.starts_with('_') && !name.starts_with("__") && !is_classvar(&a.annotation) {
                            let (mut d, mut fac): (Option<&'static ast::Expr>, Option<&'static ast::Expr>) = (None, None);
                            if let Some(v) = &a.value {
                                let v: &'static ast::Expr = v;
                                match v {
                                    ast::Expr::Call(c) if dotted_last(&c.func) == "PrivateAttr" => {
                                        d = c.arguments.args.first().or(kw(c, "default"));
                                        fac = kw(c, "default_factory");
                                    }
                                    _ => d = Some(v),
                                }
                            }
                            privates.retain(|p| p.0 != name);
                            privates.push((name.clone(), d, fac));
                            continue;
                        }
                        if name.starts_with('_') || is_classvar(&a.annotation) {
                            continue;
                        }
                        let ann = self.pyd_ann(&a.annotation, ci.module, state);
                        let optional = ann_optional(&a.annotation);
                        let mut f = PField { name: name.clone(), alias: None, ser_alias: None, alts: Vec::new(), default: None, factory: None, required: true, optional, ann };
                        if let Some(v) = &a.value {
                            let v: &'static ast::Expr = v;
                            match v {
                                ast::Expr::Call(c) if dotted_last(&c.func) == "Field" => {
                                    if let Some(d) = c.arguments.args.first() {
                                        if !is_ellipsis(d) {
                                            f.default = Some(d);
                                            f.required = false;
                                        }
                                    }
                                    if let Some(d) = kw(c, "default") {
                                        if !is_ellipsis(d) {
                                            f.default = Some(d);
                                            f.required = false;
                                        }
                                    }
                                    if let Some(d) = kw(c, "default_factory") {
                                        f.factory = Some(d);
                                        f.required = false;
                                    }
                                    let alias = kw(c, "alias").and_then(str_lit);
                                    f.alias = kw(c, "validation_alias").and_then(str_lit).or_else(|| alias.clone());
                                    // `validation_alias=AliasChoices("a", "b")`
                                    if let Some(ast::Expr::Call(ac)) = kw(c, "validation_alias") {
                                        if dotted_last(&ac.func) == "AliasChoices" {
                                            let names: Vec<String> = ac.arguments.args.iter().filter_map(str_lit).collect();
                                            if let Some((first, rest)) = names.split_first() {
                                                f.alias = Some(first.clone());
                                                f.alts = rest.to_vec();
                                            }
                                        }
                                    }
                                    f.ser_alias = kw(c, "serialization_alias").and_then(str_lit).or(alias);
                                }
                                e if is_ellipsis(e) => {}
                                _ => {
                                    f.default = Some(v);
                                    f.required = false;
                                }
                            }
                        }
                        if let Some(e) = fields.iter_mut().find(|x| x.name == name) {
                            *e = f;
                        } else {
                            fields.push(f);
                        }
                    }
                    ast::Stmt::Assign(a) => {
                        if let [ast::Expr::Name(n)] = &a.targets[..] {
                            if n.id.as_str() == "model_config" {
                                config_of(&a.value, &mut extra, &mut populate_by_name);
                            }
                        }
                    }
                    // v1: `class Config: extra = "allow"` / `Extra.allow`
                    ast::Stmt::ClassDef(inner) if inner.name.id.as_str() == "Config" => {
                        for s2 in &inner.body {
                            if let ast::Stmt::Assign(a) = s2 {
                                if let [ast::Expr::Name(n)] = &a.targets[..] {
                                    match n.id.as_str() {
                                        "extra" => extra = extra_of(&a.value).unwrap_or(extra),
                                        "allow_population_by_field_name" | "populate_by_name" => populate_by_name = bool_lit(&a.value).unwrap_or(populate_by_name),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    ast::Stmt::FunctionDef(fd) => {
                        let fname = fd.name.id.to_string();
                        let fv = self.cls_slots.get(&(c, Ns::Attr, key_str_lit(&fname))).cloned().unwrap_or_default();
                        let Some(func) = fv.pts.iter().find(|o| self.kind(*o) == Kind::Function) else { continue };
                        for d in &fd.decorator_list {
                            let (callee, call) = match &d.expression {
                                ast::Expr::Call(c2) => (&*c2.func, Some(c2)),
                                e => (e, None),
                            };
                            let dname = dotted_last(callee);
                            let args_s: Vec<String> = call.map(|c2| c2.arguments.args.iter().filter_map(str_lit).collect()).unwrap_or_default();
                            let mode = call.and_then(|c2| kw(c2, "mode")).and_then(str_lit);
                            let pre = call.and_then(|c2| kw(c2, "pre")).and_then(bool_lit).unwrap_or(false);
                            match dname.as_str() {
                                "field_validator" | "validator" => {
                                    let m = mode.unwrap_or_else(|| if pre { "before".into() } else { "after".into() });
                                    field_validators.push(Validator { func, fields: args_s, mode: m });
                                }
                                "model_validator" => {
                                    let m = mode.unwrap_or_else(|| "after".into());
                                    let v = Validator { func, fields: vec![], mode: m.clone() };
                                    if m == "after" {
                                        model_after.push(v);
                                    } else {
                                        model_before.push(v);
                                    }
                                }
                                "root_validator" => {
                                    let v = Validator { func, fields: vec![], mode: if pre { "before".into() } else { "after".into() } };
                                    if pre {
                                        model_before.push(v);
                                    } else {
                                        root_post.push(v);
                                    }
                                }
                                "computed_field" => {
                                    if !computed.contains(&fname) {
                                        computed.push(fname.clone());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if root && !fields.iter().any(|f| f.name == "root") {
            fields.push(PField { name: "root".into(), alias: None, ser_alias: None, alts: Vec::new(), default: None, factory: None, required: true, optional: false, ann: Ann::Other });
        }
        let p = Rc::new(PClass { fields, extra, populate_by_name, root, model_before, model_after, root_post, field_validators, computed, privates });
        self.pyd_classes.insert(cls, p.clone());
        p
    }

    /// The validation-relevant part of an annotation, resolved in module `module`.
    fn pyd_ann(&mut self, e: &ast::Expr, module: ObjId, state: &State) -> Ann {
        match e {
            ast::Expr::Subscript(sb) => {
                let base = type_name(&sb.value);
                let elts: Vec<&ast::Expr> = match &*sb.slice {
                    ast::Expr::Tuple(t) => t.elts.iter().collect(),
                    other => vec![other],
                };
                match base.as_str() {
                    "Optional" => Ann::Optional(Box::new(self.pyd_ann(elts[0], module, state))),
                    "Annotated" => self.pyd_ann(elts[0], module, state),
                    "list" | "List" | "Sequence" | "set" | "Set" | "tuple" | "Tuple" | "frozenset" | "FrozenSet" | "Iterable" => {
                        if base.to_lowercase().starts_with("tuple") && elts.len() != 1 && !(elts.len() == 2 && is_ellipsis(elts[1])) {
                            return Ann::Other;
                        }
                        Ann::List(Box::new(self.pyd_ann(elts[0], module, state)))
                    }
                    "dict" | "Dict" | "Mapping" if elts.len() == 2 => Ann::Dict(Box::new(self.pyd_ann(elts[1], module, state))),
                    "Union" => {
                        let non_none: Vec<&&ast::Expr> = elts.iter().filter(|x| !matches!(x, ast::Expr::NoneLiteral(_))).collect();
                        if non_none.len() == 1 {
                            let inner = self.pyd_ann(non_none[0], module, state);
                            if non_none.len() < elts.len() {
                                Ann::Optional(Box::new(inner))
                            } else {
                                inner
                            }
                        } else {
                            Ann::Other
                        }
                    }
                    _ => Ann::Other,
                }
            }
            ast::Expr::BinOp(b) if matches!(b.op, ast::Operator::BitOr) => {
                let (l, r) = (&*b.left, &*b.right);
                match (matches!(l, ast::Expr::NoneLiteral(_)), matches!(r, ast::Expr::NoneLiteral(_))) {
                    (false, true) => Ann::Optional(Box::new(self.pyd_ann(l, module, state))),
                    (true, false) => Ann::Optional(Box::new(self.pyd_ann(r, module, state))),
                    _ => Ann::Other,
                }
            }
            ast::Expr::StringLiteral(s0) => {
                // a forward reference: `"Node"`, `list["Node"]` written as a string
                let t = s0.value.to_str().trim().to_string();
                if let Ok(parsed) = ruff_python_parser::parse_expression(&t) {
                    let e2: &'static ast::Expr = Box::leak(Box::new(parsed.into_expr()));
                    return self.pyd_ann(e2, module, state);
                }
                Ann::Other
            }
            ast::Expr::Name(_) | ast::Expr::Attribute(_) => {
                let n = type_name(e);
                let v = state.slot_opt(module, Ns::Attr, key_str_lit(&n)).unwrap_or_default();
                let mut found = v.pts.iter().find(|o| self.kind(*o) == Kind::Class && is_model(self, *o));
                if found.is_none() {
                    // by name among the program's classes (this module's first): the state at
                    // hand may be a summary's, without the module's names
                    let cands: Vec<ObjId> = self.user_classes.iter().copied().filter(|c| self.class_info_opt(*c).map(|ci| ci.name == n).unwrap_or(false) && is_model(self, *c)).collect();
                    found = cands.iter().copied().find(|c| self.class_info(*c).module == module).or_else(|| cands.first().copied());
                }
                match found {
                    Some(c) => Ann::Model(c),
                    None => Ann::Other,
                }
            }
            _ => Ann::Other,
        }
    }

    /// `M(**kw)`: the input as a dict (the keyword arguments and the `**` mappings), then
    /// validation.
    pub fn pyd_construct(&mut self, cls: ObjId, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        let pc = self.pyd_class(cls, state);
        if pc.root {
            // `RootModel(value)`: the value is the root
            let data = args.first().cloned().or_else(|| kwargs.get(&s("root")).cloned()).unwrap_or_else(|| Value::data("root"));
            return self.pyd_root_instance(cls, &data, state, node);
        }
        let did = self.alloc_at(Kind::Dict, node, state, &format!("{}(…)@L{}", self.class_info(cls).name, node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("pydinput"));
        state.set_row(did, Ns::Item, Row::new());
        for sk in star_kwargs {
            self.copy_into(did, Ns::Item, sk, state, true, &KeySet::new());
        }
        for (k, v) in kwargs.iter() {
            let kk = key_str_lit(&intern::str_get(*k));
            let r = state.row(did, Ns::Item).store(kk, true);
            state.set_row(did, Ns::Item, r);
            state.set_slot(did, Ns::Item, kk, v.clone(), true);
        }
        self.pyd_validate(cls, &Value::obj(did), state, node, true)
    }

    fn pyd_root_instance(&mut self, cls: ObjId, data: &Value, state: &mut State, node: &Node) -> Value {
        let site = format!("{}:{}", crate::interp::site_string(node), self.class_info(cls).name);
        let inst = self.alloc(Kind::Instance, &site, state, &format!("{}@L{}", self.class_info(cls).name, node.line), Payload::Cls(cls), Some(Row::new()), Ns::Attr, None, None);
        let rk = key_str_lit("root");
        state.set_row(inst, Ns::Attr, Row::new().store(rk, true));
        state.set_slot(inst, Ns::Attr, rk, data.clone(), true);
        Value::obj(inst)
    }

    /// `M.model_validate(data)`: an instance of `cls` from `data` (a dict, an open input, an
    /// instance of the class, or a placeholder). `kw_input`: the input was built from the
    /// call's keywords (`M(a=…)`), so extra keys reach `extra="allow"` attributes the same way.
    pub fn pyd_validate(&mut self, cls: ObjId, data: &Value, state: &mut State, node: &Node, kw_input: bool) -> Value {
        let _ = kw_input;
        self.pyd_validate_opts(cls, data, state, node, true)
    }

    pub fn pyd_ann_pub(&mut self, e: &'static ast::Expr, module: ObjId, state: &State) -> Ann {
        self.pyd_ann(e, module, state)
    }

    pub fn pyd_convert_pub(&mut self, ann: &Ann, val: &Value, state: &mut State, node: &Node) -> Value {
        self.pyd_convert(ann, val, state, node)
    }

    pub fn pyd_dump_value_pub(&mut self, ann: &Ann, v: &Value, opts: &DumpOpts, state: &mut State, node: &Node) -> Value {
        self.pyd_dump_value(ann, v, opts, state, node)
    }

    /// `pyd_validate`; `report`: a missing required field is the program's failure (a
    /// verdict, an input's requirement) — not when FastAPI validates a test request (it
    /// answers 422 instead).
    pub fn pyd_validate_opts(&mut self, cls: ObjId, data: &Value, state: &mut State, node: &Node, report: bool) -> Value {
        let pc = self.pyd_class(cls, state);
        if pc.root {
            return self.pyd_root_instance(cls, data, state, node);
        }
        let mut outs: Vec<Value> = Vec::new();
        // an instance of the class itself validates to itself
        let (insts, rest): (Vec<ObjId>, Vec<ObjId>) = data.pts.iter().partition(|o| self.kind(*o) == Kind::Instance && self.instance_class(*o).map(|c| self.mro(c).contains(&cls)).unwrap_or(false));
        for i in insts {
            outs.push(Value::obj(i));
        }
        if rest.is_empty() && !data.pts.is_empty() {
            return join_values(outs);
        }
        let mut data = Value::with_kv(crate::domain::ObjSet::from_vec(rest), data.kv.clone());
        // `mode="before"` model validators (v1 `root_validator(pre=True)`) see the raw input
        // and return what is validated
        for v in pc.model_before.clone() {
            data = self.call_user(v.func, Some(Value::obj(cls)), vec![data.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
        }
        let site = format!("{}:{}", crate::interp::site_string(node), self.class_info(cls).name);
        let inst = self.alloc(Kind::Instance, &site, state, &format!("{}@L{}", self.class_info(cls).name, node.line), Payload::Cls(cls), Some(Row::new()), Ns::Attr, None, None);
        state.set_row(inst, Ns::Attr, Row::new());
        let recs: Vec<ObjId> = data.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Dict | Kind::Unknown | Kind::Ph | Kind::Instance | Kind::Namespace)).collect();
        let recv = Value::objs(crate::domain::ObjSet::from_vec(recs.clone()));
        let other_data = data.pts.is_empty() || data.pts.iter().any(|o| !recs.contains(&o));
        let mut field_keys: Vec<KeyId> = Vec::new();
        let (mut set_sure, mut set_maybe): (Vec<KeyId>, Vec<KeyId>) = (Vec::new(), Vec::new());
        for f in pc.fields.clone() {
            let key = f.alias.clone().unwrap_or_else(|| f.name.clone());
            {
                // `model_fields_set`: the fields the input gave
                let kk0 = key_str_lit(&key);
                let nk0 = key_str_lit(&f.name);
                let mut ks = vec![kk0];
                ks.extend(f.alts.iter().map(|a| key_str_lit(a)));
                if f.alias.is_some() && pc.populate_by_name {
                    ks.push(nk0);
                }
                let nsf = |an: &Analyzer, o: ObjId| if matches!(an.kind(o), Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
                let sure = !recs.is_empty() && !other_data && recs.iter().all(|o| ks.iter().any(|k| state.row(*o, nsf(self, *o)).definite.contains(*k)));
                let maybe = other_data || recs.iter().any(|o| ks.iter().any(|k| state.row(*o, nsf(self, *o)).present.contains(*k)) || state.row(*o, nsf(self, *o)).open());
                if sure || f.required {
                    set_sure.push(nk0);
                }
                if maybe || sure {
                    set_maybe.push(nk0);
                }
            }
            let kk = key_str_lit(&key);
            field_keys.push(kk);
            let nk = key_str_lit(&f.name);
            if f.alias.is_some() {
                field_keys.push(nk);
            }
            field_keys.extend(f.alts.iter().map(|a| key_str_lit(a)));
            let mut val = Value::new();
            if !recs.is_empty() {
                // by name too when `populate_by_name` (the alias wins when both are there)
                let mut keys = vec![kk];
                keys.extend(f.alts.iter().map(|a| key_str_lit(a)));
                if f.alias.is_some() && pc.populate_by_name {
                    keys.push(nk);
                }
                let ns_of = |an: &Analyzer, o: ObjId| if matches!(an.kind(o), Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
                let definite = recs.iter().all(|o| keys.iter().any(|k| state.row(*o, ns_of(self, *o)).definite.contains(*k)));
                let present = recs.iter().any(|o| keys.iter().any(|k| state.row(*o, ns_of(self, *o)).present.contains(*k) || state.row(*o, ns_of(self, *o)).open()));
                if f.required && !definite && keys.len() == 1 && report {
                    // a required field: a read (missing → ValidationError; an input's requirement)
                    let ns = ns_of(self, recs[0]);
                    val = self.load_item_or_attr(&recv, ns, kk, node, state);
                } else if f.required && !definite {
                    // the alias or the name (`populate_by_name`): per record, missing when it
                    // has neither; an open input needs one of them (each on some paths)
                    if report {
                        for &o in &recs {
                            let row = state.row(o, ns_of(self, o));
                            if row.definite.contains(kk) {
                                self.verdict(node, kk, &Classification::new(Tier::Proven), o, Ns::Item, None, None);
                            } else if !row.open() && !keys.iter().any(|k| row.present.contains(*k)) {
                                self.verdict(node, kk, &Classification::with_note(Tier::Missing, "pydantic field required"), o, Ns::Item, None, None);
                            }
                        }
                        let open: Vec<ObjId> = recs.iter().copied().filter(|o| state.row(*o, ns_of(self, *o)).open() && !keys.iter().any(|k| state.row(*o, ns_of(self, *o)).definite.contains(*k))).collect();
                        if !open.is_empty() {
                            let ov = Value::objs(crate::domain::ObjSet::from_vec(open.clone()));
                            let mut branches: Vec<State> = Vec::new();
                            for k in &keys {
                                let mut b = state.copy();
                                let ns = ns_of(self, open[0]);
                                let saved = std::mem::replace(&mut self.quiet, true);
                                self.branch_depth += 1;
                                let x = self.load_item_or_attr(&ov, ns, *k, node, &mut b);
                                self.branch_depth -= 1;
                                self.quiet = saved;
                                val = val.join(&x);
                                branches.push(b);
                            }
                            let mut joined = branches.remove(0);
                            for mut b in branches {
                                joined = joined.join(&mut b);
                            }
                            state.replace_with(joined);
                        }
                    }
                    for k in &keys {
                        val = val.join(&self.pyd_read_guarded(&recs, *k, state));
                    }
                } else {
                    for k in &keys {
                        if recs.iter().any(|o| state.row(*o, ns_of(self, *o)).present.contains(*k) || state.row(*o, ns_of(self, *o)).open()) {
                            val = val.join(&self.pyd_read_guarded(&recs, *k, state));
                        }
                    }
                    if !definite {
                        val = val.join(&self.pyd_default(&f, node, state));
                    } else if val.empty() {
                        val = self.pyd_default(&f, node, state);
                    }
                }
                let _ = present;
            } else if !f.required {
                val = self.pyd_default(&f, node, state);
            }
            if other_data && !f.required {
                val = val.join(&self.pyd_default(&f, node, state));
            }
            if val.empty() {
                val = Value::data(&format!("{}.{}", self.class_info(cls).name, f.name));
            }
            // field validators before the type's conversion, then the conversion, then after
            for v in pc.field_validators.iter().filter(|v| (v.fields.contains(&f.name) || v.fields.iter().any(|x| x == "*")) && matches!(v.mode.as_str(), "before" | "wrap" | "plain")).cloned().collect::<Vec<_>>() {
                if v.mode == "wrap" {
                    continue;
                }
                val = self.pyd_call_validator(cls, &v, &val, state, node);
            }
            val = self.pyd_convert(&f.ann, &val, state, node);
            for v in pc.field_validators.iter().filter(|v| (v.fields.contains(&f.name) || v.fields.iter().any(|x| x == "*")) && v.mode == "after").cloned().collect::<Vec<_>>() {
                val = self.pyd_call_validator(cls, &v, &val, state, node);
            }
            let r = state.row(inst, Ns::Attr).store(nk, true);
            state.set_row(inst, Ns::Attr, r);
            state.set_slot(inst, Ns::Attr, nk, val, true);
        }
        let entry = (KeySet::from_iter(set_sure), KeySet::from_iter(set_maybe));
        let joined = match self.pyd_set.get(&inst) {
            Some((s0, m0)) if !self.obj(inst).single => (s0.intersection(&entry.0), m0.union(&entry.1)),
            _ => entry,
        };
        self.pyd_set.insert(inst, joined);
        // extra keys: attributes with `extra="allow"`
        if pc.extra == Extra::Allow {
            for &o in &recs {
                let ns = if matches!(self.kind(o), Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
                let row = state.row(o, ns);
                let kill: KeySet = KeySet::from_iter(field_keys.iter().copied());
                let mut r = state.row(inst, Ns::Attr);
                for k in row.present.iter() {
                    if kill.contains(k) {
                        continue;
                    }
                    r = r.store(k, row.definite.contains(k) && recs.len() == 1);
                    state.set_slot(inst, Ns::Attr, k, state.slot(o, ns, k), recs.len() == 1);
                }
                if row.open() {
                    r = r.copy_from(&row, false, &kill, intern::EMPTY_LENS);
                }
                state.set_row(inst, Ns::Attr, r);
            }
            if other_data {
                let r = state.row(inst, Ns::Attr).data_store(crate::domain::Source::new(&format!("{}@L{}", self.class_info(cls).name, node.line), crate::domain::SourceKind::Data));
                state.set_row(inst, Ns::Attr, r);
            }
        }
        // v1 `@root_validator` (post): the dict of values in, a dict out
        for v in pc.root_post.clone() {
            let vals = self.pyd_dump(inst, &DumpOpts::default(), state, node);
            let out = self.call_user(v.func, Some(Value::obj(cls)), vec![vals], IndexMap::new(), vec![], vec![], state, node, None, false);
            for d in out.pts.iter().filter(|o| self.kind(*o) == Kind::Dict).collect::<Vec<_>>() {
                for f in pc.fields.iter() {
                    let nk = key_str_lit(&f.name);
                    if state.row(d, Ns::Item).present.contains(nk) {
                        let nv = state.slot(d, Ns::Item, nk);
                        state.set_slot(inst, Ns::Attr, nk, nv, false);
                    }
                }
            }
        }
        self.pyd_init_privates(cls, inst, state, node);
        for v in pc.model_after.clone() {
            self.call_user(v.func, Some(Value::obj(inst)), vec![], IndexMap::new(), vec![], vec![], state, node, None, false);
        }
        outs.push(Value::obj(inst));
        join_values(outs)
    }

    /// Private attributes get their defaults, then a user `model_post_init` runs.
    fn pyd_init_privates(&mut self, cls: ObjId, inst: ObjId, state: &mut State, node: &Node) {
        let pc = self.pyd_class(cls, state);
        for (name, d, fac) in pc.privates.clone() {
            let v = match (fac, d) {
                (Some(f), _) => {
                    let fv = self.eval(f, state);
                    self.call_value(&fv, node, None, state, Some(vec![]))
                }
                (None, Some(d)) => self.eval(d, state),
                _ => continue,
            };
            state.set_slot(inst, Ns::Attr, priv_key(&name), v, true);
        }
        if let Some(defc) = self.class_lookup(cls, "model_post_init") {
            let fv = self.cls_slots.get(&(defc, Ns::Attr, key_str_lit("model_post_init"))).cloned().unwrap_or_default();
            for f in fv.pts.iter().filter(|f| self.kind(*f) == Kind::Function).collect::<Vec<_>>() {
                self.call_user(f, Some(Value::obj(inst)), vec![self.none_v.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
            }
        }
    }

    /// Is `name` a private attribute of model class `cls`?
    pub fn pyd_is_private(&mut self, cls: ObjId, name: &str, state: &State) -> bool {
        name.starts_with('_') && !name.starts_with("__") && is_model(self, cls) && self.pyd_class(cls, state).privates.iter().any(|p| p.0 == name)
    }

    /// A required field's read: `d[k]` on a dict / input, `o.k` on an object (verdicts and
    /// requirements as for the program's own reads).
    fn load_item_or_attr(&mut self, recv: &Value, ns: Ns, k: KeyId, node: &Node, state: &mut State) -> Value {
        if ns == Ns::Attr {
            return self.load_attr(recv, &intern::key_py_str(k), node, state, true);
        }
        self.load_item(recv, Ns::Item, &Value::lit(k), node, state, true)
    }

    /// A guarded read of `k` from the records (no verdict, no requirement): the values there.
    fn pyd_read_guarded(&mut self, recs: &[ObjId], k: KeyId, state: &mut State) -> Value {
        let mut out = Value::new();
        for &o in recs {
            let ns = if matches!(self.kind(o), Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
            let row = state.row(o, ns);
            if row.present.contains(k) {
                out = out.join(&state.slot(o, ns, k));
            } else if row.open() {
                let v = self.path_placeholder_value(o, ns, k, &row, state);
                out = out.join(&v);
            }
        }
        out
    }

    fn pyd_default(&mut self, f: &PField, node: &Node, state: &mut State) -> Value {
        if let Some(fac) = f.factory {
            let fv = self.eval(fac, state);
            return self.call_value(&fv, node, None, state, Some(vec![]));
        }
        match f.default {
            Some(d) => self.eval(d, state),
            None => Value::new(),
        }
    }

    fn pyd_call_validator(&mut self, cls: ObjId, v: &Validator, val: &Value, state: &mut State, node: &Node) -> Value {
        // field validators are classmethods: (cls, value[, info])
        let nparams = match &self.func_info(v.func).node {
            crate::interp::FuncNode::Def(f) => crate::scopes::all_params(&f.parameters).len(),
            _ => 2,
        };
        let mut args = vec![val.clone()];
        if nparams >= 3 {
            args.push(Value::data("ValidationInfo"));
        }
        let out = self.call_user(v.func, Some(Value::obj(cls)), args, IndexMap::new(), vec![], vec![], state, node, None, false);
        if out.empty() {
            val.clone()
        } else {
            out
        }
    }

    /// The value after validation by its declared type: a nested model from a dict, a list
    /// of models from a list of dicts; other values pass.
    fn pyd_convert(&mut self, ann: &Ann, val: &Value, state: &mut State, node: &Node) -> Value {
        match ann {
            Ann::Model(c) => {
                let c = *c;
                // None / literals pass (a validation error otherwise: not this model's concern)
                let recs: Vec<ObjId> = val.pts.iter().collect();
                if recs.is_empty() {
                    return val.clone();
                }
                let lits = Value::kv_only(val.kv.clone());
                let mut out = self.pyd_validate(c, &Value::objs(crate::domain::ObjSet::from_vec(recs)), state, node, false);
                if !val.kv.is_bot() && val.kv.is_literal() {
                    out = out.join(&lits);
                }
                out
            }
            Ann::Optional(inner) => self.pyd_convert(inner, val, state, node),
            Ann::List(inner) if matches!(**inner, Ann::Model(_) | Ann::Optional(_)) => {
                let mut outs: Vec<Value> = Vec::new();
                for o in val.pts.iter() {
                    if !matches!(self.kind(o), Kind::List | Kind::Tuple | Kind::Set | Kind::Unknown | Kind::Ph) {
                        outs.push(Value::obj(o));
                        continue;
                    }
                    let it = self.iterate(&Value::obj(o), node, state);
                    match it.items {
                        Some(items) => {
                            let conv: Vec<Value> = items.iter().map(|x| self.pyd_convert(inner, x, state, node)).collect();
                            outs.push(self.alloc_positional(Kind::List, node, Some(conv), Value::new(), crate::domain::DATA, state));
                        }
                        None => {
                            let e = match it.elem {
                                crate::interp::funcs::Elem::One(v) => v,
                                _ => Value::data("elem"),
                            };
                            let ce = self.pyd_convert(inner, &e, state, node);
                            outs.push(self.alloc_positional(Kind::List, node, None, ce, crate::domain::DATA, state));
                        }
                    }
                }
                if val.pts.is_empty() {
                    return val.clone();
                }
                join_values(outs)
            }
            Ann::Dict(inner) if matches!(**inner, Ann::Model(_)) => {
                let mut outs: Vec<Value> = Vec::new();
                for o in val.pts.iter() {
                    if self.kind(o) != Kind::Dict {
                        outs.push(Value::obj(o));
                        continue;
                    }
                    let nid = self.alloc_at(Kind::Dict, node, state, &format!("dict@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("pyddict"));
                    let row = state.row(o, Ns::Item);
                    state.set_row(nid, Ns::Item, row.clone());
                    for k in row.present.iter() {
                        let v = state.slot(o, Ns::Item, k);
                        let cv = self.pyd_convert(inner, &v, state, node);
                        state.set_slot(nid, Ns::Item, k, cv, true);
                    }
                    outs.push(Value::obj(nid));
                }
                if val.pts.is_empty() {
                    return val.clone();
                }
                join_values(outs)
            }
            _ => val.clone(),
        }
    }
}

#[derive(Clone, Default)]
pub struct DumpOpts {
    pub include: Option<KeySet>,
    pub exclude: Option<KeySet>,
    /// nested include / exclude specs by field
    pub sub_include: Vec<(KeyId, DumpOpts)>,
    pub exclude_none: bool,
    pub exclude_unset: bool,
    pub exclude_defaults: bool,
    pub by_alias: bool,
}

/// The key of a literal default (`"light"`, `12`, `None`, `True`, `-1`).
fn literal_key(e: &ast::Expr) -> Option<KeyId> {
    Some(match e {
        ast::Expr::StringLiteral(s0) => key_str_lit(s0.value.to_str()),
        ast::Expr::NoneLiteral(_) => intern::key_none(),
        ast::Expr::BooleanLiteral(b) => intern::key_bool(b.value),
        ast::Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(i) => intern::key_int(i.as_i64()?),
            _ => return None,
        },
        ast::Expr::UnaryOp(u) if matches!(u.op, ast::UnaryOp::USub) => match &*u.operand {
            ast::Expr::NumberLiteral(n) => match &n.value {
                ast::Number::Int(i) => intern::key_int(-i.as_i64()?),
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    })
}

fn extra_of(e: &ast::Expr) -> Option<Extra> {
    let t = match e {
        ast::Expr::StringLiteral(s0) => s0.value.to_str().to_string(),
        ast::Expr::Attribute(a) => a.attr.id.to_string(),
        _ => return None,
    };
    Some(match t.as_str() {
        "allow" => Extra::Allow,
        "forbid" => Extra::Forbid,
        "ignore" => Extra::Ignore,
        _ => return None,
    })
}

fn config_of(v: &ast::Expr, extra: &mut Extra, populate: &mut bool) {
    match v {
        ast::Expr::Call(c) => {
            if let Some(e) = kw(c, "extra").and_then(extra_of) {
                *extra = e;
            }
            if let Some(b) = kw(c, "populate_by_name").and_then(bool_lit) {
                *populate = b;
            }
        }
        ast::Expr::Dict(d) => {
            for item in d.items.iter() {
                let Some(k) = item.key.as_ref().and_then(str_lit) else { continue };
                match k.as_str() {
                    "extra" => {
                        if let Some(e) = extra_of(&item.value) {
                            *extra = e;
                        }
                    }
                    "populate_by_name" => {
                        if let Some(b) = bool_lit(&item.value) {
                            *populate = b;
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn is_classvar(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::Subscript(sb) => type_name(&sb.value) == "ClassVar",
        other => type_name(other) == "ClassVar",
    }
}

fn ann_optional(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::NoneLiteral(_) => true,
        ast::Expr::Subscript(sb) => {
            let base = type_name(&sb.value);
            match base.as_str() {
                "Optional" => true,
                "Union" => match &*sb.slice {
                    ast::Expr::Tuple(t) => t.elts.iter().any(ann_optional),
                    other => ann_optional(other),
                },
                "Annotated" => match &*sb.slice {
                    ast::Expr::Tuple(t) => t.elts.first().map(ann_optional).unwrap_or(false),
                    other => ann_optional(other),
                },
                _ => false,
            }
        }
        ast::Expr::BinOp(b) if matches!(b.op, ast::Operator::BitOr) => ann_optional(&b.left) || ann_optional(&b.right),
        ast::Expr::Name(n) => matches!(n.id.as_str(), "Any" | "None"),
        ast::Expr::Attribute(a) => a.attr.id.as_str() == "Any",
        ast::Expr::StringLiteral(s0) => {
            let t = s0.value.to_str();
            t.contains("None") || t.starts_with("Optional")
        }
        _ => false,
    }
}

impl Analyzer {
    /// `model_dump()` of a model instance (or a placeholder typed with a model class).
    pub fn pyd_dump(&mut self, oid: ObjId, opts: &DumpOpts, state: &mut State, node: &Node) -> Value {
        if self.pyd_depth > 6 {
            let did = self.alloc_at(Kind::Dict, node, state, &format!("model_dump@L{}", node.line), Payload::None, None, Ns::Item, None, Some("dumpdeep"));
            state.set_row(did, Ns::Item, Row::new().data_store(crate::domain::Source::new(&format!("model_dump@L{}", node.line), crate::domain::SourceKind::Data)));
            return Value::obj(did);
        }
        self.pyd_depth += 1;
        let out = self.pyd_dump_inner(oid, opts, state, node);
        self.pyd_depth -= 1;
        out
    }

    fn pyd_dump_inner(&mut self, oid: ObjId, opts: &DumpOpts, state: &mut State, node: &Node) -> Value {
        let cls = match self.kind(oid) {
            Kind::Instance => self.instance_class(oid),
            Kind::Ph => self.ph_info(oid).cls.get(),
            _ => None,
        };
        let did = self.alloc_at(Kind::Dict, node, state, &format!("model_dump@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("dump{}", oid)));
        state.set_row(did, Ns::Item, Row::new());
        let Some(cls) = cls.filter(|c| is_model(self, *c)) else {
            let r = Row::new().data_store(crate::domain::Source::new(&format!("model_dump@L{}", node.line), crate::domain::SourceKind::Data));
            state.set_row(did, Ns::Item, r);
            return Value::obj(did);
        };
        let pc = self.pyd_class(cls, state);
        let mut order: Vec<KeyId> = Vec::new();
        let mut row = Row::new();
        let ph = self.kind(oid) == Kind::Ph;
        let mut names: Vec<(String, Option<String>, bool, Ann, bool)> = pc.fields.iter().map(|f| (f.name.clone(), f.ser_alias.clone(), f.optional, f.ann.clone(), f.required)).collect();
        for c in &pc.computed {
            names.push((c.clone(), None, false, Ann::Other, true));
        }
        for (name, ser, optional, ann, required) in names {
            let nk = key_str_lit(&name);
            if opts.include.as_ref().map(|i| !i.contains(nk)).unwrap_or(false) || opts.exclude.as_ref().map(|e| e.contains(nk)).unwrap_or(false) {
                continue;
            }
            let out_key = if opts.by_alias { ser.clone().unwrap_or_else(|| name.clone()) } else { name.clone() };
            let ok = key_str_lit(&out_key);
            let v = if ph || pc.computed.contains(&name) {
                self.load_attr(&Value::obj(oid), &name, node, state, false)
            } else {
                state.slot(oid, Ns::Attr, nk)
            };
            let sub = opts.sub_include.iter().find(|(k, _)| *k == nk).map(|(_, o)| o.clone()).unwrap_or_else(|| DumpOpts { by_alias: opts.by_alias, exclude_none: opts.exclude_none, exclude_unset: opts.exclude_unset, exclude_defaults: opts.exclude_defaults, ..Default::default() });
            let dv = self.pyd_dump_value(&ann, &v, &sub, state, node);
            // per option: Some(true) kept, Some(false) left out, None either
            let none_lit = intern::key_none();
            let only_none = v.pts.is_empty() && v.kv.as_lit() == Some(none_lit);
            let kv_none = v.kv.as_lit() == Some(none_lit) || matches!(&v.kv, KeyVal::Lits(ks) if ks.contains(none_lit));
            let unknown_part = (!v.kv.is_literal() && !v.kv.is_bot()) || v.pts.iter().any(|o| matches!(self.kind(o), Kind::Unknown | Kind::Ph | Kind::External));
            let maybe_none = kv_none || (unknown_part && (optional || matches!(ann, Ann::Optional(_))));
            let mut keep: Option<bool> = Some(true);
            let mut meet = |x: Option<bool>| {
                keep = match (keep, x) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), Some(true)) => Some(true),
                    _ => None,
                }
            };
            if opts.exclude_none {
                meet(if only_none { Some(false) } else if maybe_none { None } else { Some(true) });
            }
            if opts.exclude_unset && !pc.computed.contains(&name) {
                meet(match self.pyd_set.get(&oid) {
                    Some((sure, maybe)) if !ph => {
                        if sure.contains(nk) {
                            Some(true)
                        } else if maybe.contains(nk) {
                            None
                        } else {
                            Some(false)
                        }
                    }
                    _ => if required { Some(true) } else { None },
                });
            }
            if opts.exclude_defaults && !pc.computed.contains(&name) {
                let dflt = pc.fields.iter().find(|f| f.name == name).and_then(|f| if f.factory.is_some() { None } else { f.default }).and_then(literal_key);
                meet(if required {
                    Some(true)
                } else {
                    match (dflt, v.pts.is_empty(), &v.kv) {
                        (Some(d), true, KeyVal::Lit(k)) => Some(*k != d),
                        (Some(d), true, KeyVal::Lits(ks)) if !ks.contains(d) => Some(true),
                        (Some(_), false, KeyVal::Bot) => Some(true),
                        _ => None,
                    }
                });
            }
            if keep == Some(false) {
                continue;
            }
            row = row.store(ok, keep == Some(true));
            order.push(ok);
            state.set_slot(did, Ns::Item, ok, dv, true);
        }
        // extra attributes (`extra="allow"`)
        if pc.extra == Extra::Allow && !ph {
            let ir = state.row(oid, Ns::Attr);
            let fieldset: KeySet = KeySet::from_iter(pc.fields.iter().map(|f| key_str_lit(&f.name)));
            for k in ir.present.iter() {
                if fieldset.contains(k) || opts.exclude.as_ref().map(|e| e.contains(k)).unwrap_or(false) || opts.include.as_ref().map(|i| !i.contains(k)).unwrap_or(false) {
                    continue;
                }
                row = row.store(k, ir.definite.contains(k));
                state.set_slot(did, Ns::Item, k, state.slot(oid, Ns::Attr, k), true);
            }
            if ir.open() && opts.include.is_none() {
                row = row.copy_from(&ir, false, &fieldset, intern::EMPTY_LENS);
            }
        }
        state.set_row(did, Ns::Item, row.with_order(order));
        Value::obj(did)
    }

    fn pyd_dump_value(&mut self, ann: &Ann, v: &Value, opts: &DumpOpts, state: &mut State, node: &Node) -> Value {
        let mut outs: Vec<Value> = Vec::new();
        if !v.kv.is_bot() {
            outs.push(Value::kv_only(v.kv.clone()));
        }
        for o in v.pts.iter() {
            if is_instance(self, o) {
                outs.push(self.pyd_dump(o, opts, state, node));
                continue;
            }
            if self.kind(o) == Kind::Ph && self.ph_info(o).cls.get().map(|c| is_model(self, c)).unwrap_or(false) {
                outs.push(self.pyd_dump(o, opts, state, node));
                continue;
            }
            match (self.kind(o), ann) {
                (Kind::List | Kind::Tuple | Kind::Set, Ann::List(inner) | Ann::Optional(inner)) => {
                    let inner = match &**inner {
                        Ann::List(i) => (**i).clone(),
                        other => other.clone(),
                    };
                    let it = self.iterate(&Value::obj(o), node, state);
                    match it.items {
                        Some(items) => {
                            let conv: Vec<Value> = items.iter().map(|x| self.pyd_dump_value(&inner, x, opts, state, node)).collect();
                            outs.push(self.alloc_positional(Kind::List, node, Some(conv), Value::new(), crate::domain::DATA, state));
                        }
                        None => {
                            let e = match it.elem {
                                crate::interp::funcs::Elem::One(v) => v,
                                _ => Value::data("elem"),
                            };
                            let ce = self.pyd_dump_value(&inner, &e, opts, state, node);
                            outs.push(self.alloc_positional(Kind::List, node, None, ce, crate::domain::DATA, state));
                        }
                    }
                }
                (Kind::Dict, Ann::Dict(inner)) if matches!(**inner, Ann::Model(_)) => {
                    let nid = self.alloc_at(Kind::Dict, node, state, &format!("dict@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("dumpd{}", o)));
                    let row = state.row(o, Ns::Item);
                    state.set_row(nid, Ns::Item, row.clone());
                    for k in row.present.iter() {
                        let x = state.slot(o, Ns::Item, k);
                        let cx = self.pyd_dump_value(inner, &x, opts, state, node);
                        state.set_slot(nid, Ns::Item, k, cx, true);
                    }
                    outs.push(Value::obj(nid));
                }
                _ => outs.push(Value::obj(o)),
            }
        }
        join_values(outs)
    }
}

/// Literal key sets of `include=` / `exclude=`: a set / list / tuple of names, or a dict
/// (`{"addr": {"zip"}}`: nested, `{"name": True}`: the field).
fn key_spec(an: &mut Analyzer, st: &mut State, v: &Value, node: &Node) -> Option<(KeySet, Vec<(KeyId, Value)>)> {
    let _ = node;
    let mut ks: Vec<KeyId> = Vec::new();
    let mut nested: Vec<(KeyId, Value)> = Vec::new();
    for o in v.pts.iter() {
        match an.kind(o) {
            Kind::Set | Kind::List | Kind::Tuple => {
                let it = an.iterate(&Value::obj(o), node, st);
                for x in it.items? {
                    ks.push(x.kv.as_lit()?);
                }
            }
            Kind::Dict => {
                let row = st.row(o, Ns::Item);
                if row.open() || row.present != row.definite {
                    return None;
                }
                for k in row.present.iter() {
                    ks.push(k);
                    let sub = st.slot(o, Ns::Item, k);
                    if !sub.pts.is_empty() {
                        nested.push((k, sub));
                    }
                }
            }
            _ => return None,
        }
    }
    if v.pts.is_empty() {
        return None;
    }
    Some((KeySet::from_iter(ks), nested))
}

fn dump_opts(an: &mut Analyzer, st: &mut State, c: &Call) -> DumpOpts {
    let flag = |c: &Call, n: &str| c.kwargs.get(&s(n)).map(|v| v.kv.as_lit() == Some(intern::key_bool(true))).unwrap_or(false);
    let mut o = DumpOpts { exclude_none: flag(c, "exclude_none"), exclude_unset: flag(c, "exclude_unset"), exclude_defaults: flag(c, "exclude_defaults"), by_alias: flag(c, "by_alias"), ..Default::default() };
    if let Some(v) = c.kwargs.get(&s("include")).cloned() {
        match key_spec(an, st, &v, &c.node) {
            Some((ks, nested)) => {
                o.include = Some(ks);
                for (k, sub) in nested {
                    if let Some((sk, _)) = key_spec(an, st, &sub, &c.node) {
                        o.sub_include.push((k, DumpOpts { include: Some(sk), by_alias: o.by_alias, ..Default::default() }));
                    }
                }
            }
            // an include set rowan cannot read: any field may be left out
            None => o.exclude_defaults = true,
        }
    }
    if let Some(v) = c.kwargs.get(&s("exclude")).cloned() {
        match key_spec(an, st, &v, &c.node) {
            Some((ks, nested)) => {
                // `{"addr": {"zip"}}` excludes inside the field, not the field
                let whole: Vec<KeyId> = ks.iter().filter(|k| !nested.iter().any(|(n, sub)| n == k && !sub.pts.is_empty() && sub.kv.is_bot())).collect();
                o.exclude = Some(KeySet::from_iter(whole));
                for (k, sub) in nested {
                    if let Some((sk, _)) = key_spec(an, st, &sub, &c.node) {
                        o.sub_include.push((k, DumpOpts { exclude: Some(sk), by_alias: o.by_alias, ..Default::default() }));
                    }
                }
            }
            None => o.exclude_defaults = true,
        }
    }
    o
}

/// The model classes a receiver value stands for (a class, an instance, a typed placeholder).
fn recv_classes(an: &Analyzer, v: &Value) -> Vec<ObjId> {
    let mut out = Vec::new();
    for o in v.pts.iter() {
        let c = match an.kind(o) {
            Kind::Class => Some(o),
            Kind::Instance => an.instance_class(o),
            Kind::Ph => an.ph_info(o).cls.get(),
            _ => None,
        };
        if let Some(c) = c.filter(|c| is_model(an, *c)) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

/// Methods of model instances: `pydantic.<name>` (receiver: the instance).
pub fn m_instance(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let recv = c.recv.clone().unwrap_or_default();
    match name.as_str() {
        "model_dump" | "dict" => {
            let opts = dump_opts(an, st, c);
            join_values(recv.pts.iter().map(|o| an.pyd_dump(o, &opts, st, &c.node)).collect::<Vec<_>>())
        }
        "model_dump_json" | "json" => {
            let ids: Vec<String> = recv.pts.iter().map(|o| o.to_string()).collect();
            Value::data(&format!("pydjson:{}", ids.join(",")))
        }
        "model_copy" | "copy" => {
            let mut outs = Vec::new();
            for o in recv.pts.iter() {
                let cp = an.copy_object(o, &c.node, st, matches!(c.kwargs.get(&s("deep")).and_then(|v| v.kv.as_lit()), Some(k) if k == intern::key_bool(true)), &mut IndexMap::new());
                if let Some(u) = c.kwargs.get(&s("update")).cloned() {
                    // `update=` sets attributes without validation
                    an.copy_into(cp, Ns::Attr, &u, st, true, &KeySet::new());
                }
                outs.push(Value::obj(cp));
            }
            join_values(outs)
        }
        "model_fields_set" | "__fields_set__" => {
            let mut outs = Vec::new();
            for o in recv.pts.iter() {
                match an.pyd_set.get(&o).cloned() {
                    Some((sure, maybe)) if sure == maybe => {
                        let items: Vec<Value> = intern::sorted_by_str(sure.iter()).into_iter().map(Value::lit).collect();
                        let kv = KeyVal::lits(sure.clone());
                        let v = an.alloc_positional(Kind::Set, &c.node, Some(items), Value::new(), kv, st);
                        outs.push(v);
                    }
                    Some((_, maybe)) => {
                        let sid = an.alloc_simple(Kind::Set, &c.node, st, &format!("fields_set@L{}", c.line));
                        st.elems.insert(sid, KeyVal::lits(maybe));
                        outs.push(Value::obj(sid));
                    }
                    None => outs.push(Value::obj(an.alloc_simple(Kind::Set, &c.node, st, &format!("fields_set@L{}", c.line)))),
                }
            }
            join_values(outs)
        }
        "model_fields" | "__fields__" | "model_computed_fields" | "model_json_schema" | "schema" | "model_validate" | "model_validate_json" | "parse_obj" | "parse_raw" | "model_construct" | "construct" => m_class(an, st, c),
        "model_extra" => {
            let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("model_extra@L{}", c.line));
            let r = Row::new().data_store(crate::domain::Source::new(&format!("model_extra@L{}", c.line), crate::domain::SourceKind::Data));
            st.set_row(oid, Ns::Item, r);
            Value::obj(oid)
        }
        "model_post_init" => an.none_v.clone(),
        _ => Value::data(&format!("pydantic.{}", name)),
    }
}

/// Class-level API: `pydantic.cls.<name>` (receiver: the class) and the classmethods reached
/// through an instance.
pub fn m_class(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let recv = c.recv.clone().unwrap_or_default();
    let classes = recv_classes(an, &recv);
    let mut outs: Vec<Value> = Vec::new();
    for cls in classes {
        let v = match name.as_str() {
            "model_validate" | "parse_obj" | "validate" => {
                let data = c.arg(0, Some("obj")).unwrap_or_else(|| Value::data("obj"));
                an.pyd_validate(cls, &data, st, &c.node, false)
            }
            "model_validate_json" | "parse_raw" | "model_validate_strings" => {
                let text = c.arg(0, Some("json_data")).unwrap_or_else(|| Value::data("json"));
                let jc = Call { qual: "json.loads".into(), recv: None, args: vec![text], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: None, site: c.site.clone(), line: c.line };
                let data = crate::models::json_loads(an, st, &jc);
                an.pyd_validate(cls, &data, st, &c.node, false)
            }
            "model_construct" | "construct" => {
                // no validation: the fields given, the defaults of the others
                let pc = an.pyd_class(cls, st);
                let site = format!("{}:{}", crate::interp::site_string(&c.node), an.class_info(cls).name);
                let inst = an.alloc(Kind::Instance, &site, st, &format!("{}@L{}", an.class_info(cls).name, c.line), Payload::Cls(cls), Some(Row::new()), Ns::Attr, None, None);
                let mut row = Row::new();
                for f in pc.fields.clone() {
                    let nk = key_str_lit(&f.name);
                    if let Some(v) = c.kwargs.get(&s(&f.name)) {
                        row = row.store(nk, true);
                        st.set_slot(inst, Ns::Attr, nk, v.clone(), true);
                    } else if !f.required {
                        row = row.store(nk, true);
                        let d = an.pyd_default(&f, &c.node, st);
                        st.set_slot(inst, Ns::Attr, nk, d, true);
                    }
                }
                st.set_row(inst, Ns::Attr, row);
                for sk in c.star_kwargs.clone() {
                    an.copy_into(inst, Ns::Attr, &sk, st, true, &KeySet::new());
                }
                Value::obj(inst)
            }
            "model_fields" | "__fields__" | "model_computed_fields" => {
                let pc = an.pyd_class(cls, st);
                let names: Vec<String> = if name == "model_computed_fields" { pc.computed.clone() } else { pc.fields.iter().map(|f| f.name.clone()).collect() };
                let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("{}@L{}", name, c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("mf{}", cls)));
                let ks: Vec<KeyId> = names.iter().map(|n| key_str_lit(n)).collect();
                let mut row = Row::new();
                for k in &ks {
                    row = row.store(*k, true);
                    st.set_slot(oid, Ns::Item, *k, Value::data("FieldInfo"), true);
                }
                st.set_row(oid, Ns::Item, row.with_order(ks));
                Value::obj(oid)
            }
            "model_json_schema" | "schema" => {
                let pc = an.pyd_class(cls, st);
                let by_alias = !matches!(c.kwargs.get(&s("by_alias")).and_then(|v| v.kv.as_lit()), Some(k) if k == intern::key_bool(false));
                let sid = an.alloc_at(Kind::Dict, &c.node, st, &format!("schema@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("js{}", cls)));
                let pid = an.alloc_at(Kind::Dict, &c.node, st, &format!("properties@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("jsp{}", cls)));
                let mut prow = Row::new();
                let mut req: Vec<Value> = Vec::new();
                for f in pc.fields.iter() {
                    let key = if by_alias { f.alias.clone().or(f.ser_alias.clone()).unwrap_or_else(|| f.name.clone()) } else { f.name.clone() };
                    let k = key_str_lit(&key);
                    prow = prow.store(k, true);
                    let fd = an.alloc_at(Kind::Dict, &c.node, st, &format!("property@L{}", c.line), Payload::None, None, Ns::Item, None, Some(&format!("jsf{}:{}", cls, key)));
                    st.set_row(fd, Ns::Item, Row::new().data_store(crate::domain::Source::new(&format!("schema@L{}", c.line), crate::domain::SourceKind::Data)));
                    st.set_slot(pid, Ns::Item, k, Value::obj(fd), true);
                    if f.required {
                        req.push(Value::lit(k));
                    }
                }
                st.set_row(pid, Ns::Item, prow);
                let rl = an.alloc_positional(Kind::List, &c.node, Some(req), Value::new(), crate::domain::DATA, st);
                let mut srow = Row::new();
                for (k, v) in [("properties", Value::obj(pid)), ("required", rl), ("title", Value::lit_str(&an.class_info(cls).name.clone())), ("type", Value::lit_str("object"))] {
                    let kk = key_str_lit(k);
                    srow = srow.store(kk, k != "required" || pc.fields.iter().any(|f| f.required));
                    st.set_slot(sid, Ns::Item, kk, v, true);
                }
                // `$defs` / `description` and others: present when nested models / docstrings
                srow = srow.data_store(crate::domain::Source::new(&format!("schema@L{}", c.line), crate::domain::SourceKind::Data));
                st.set_row(sid, Ns::Item, srow);
                Value::obj(sid)
            }
            _ => Value::data(&format!("pydantic.{}", name)),
        };
        outs.push(v);
    }
    if outs.is_empty() {
        return Value::data(&format!("pydantic.{}", name));
    }
    join_values(outs)
}

/// `TypeAdapter(T)`: remembers T; `validate_python(data)` / `validate_json(text)` validate.
pub fn m_type_adapter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(ast_call) = c.call_ast else { return Value::data("TypeAdapter") };
    let Some(t) = ast_call.arguments.args.first() else { return Value::data("TypeAdapter") };
    let module = an.frame().module;
    let ann = an.pyd_ann(t, module, st);
    let idx = an.pyd_adapters.len();
    an.pyd_adapters.push(ann);
    Value::obj(an.alloc_raw(Kind::Namespace, &format!("typeadapter:{}", idx), "", true, "TypeAdapter", Payload::Method(format!("typeadapter.{}", idx), Value::new())))
}

/// `adapter.validate_python(data)` and friends (`typeadapter.<idx>.<method>`).
pub fn adapter_call(an: &mut Analyzer, st: &mut State, idx: usize, method: &str, c: &Call) -> Value {
    let ann = an.pyd_adapters.get(idx).cloned().unwrap_or(Ann::Other);
    match method {
        "validate_python" | "validate_strings" => {
            let data = c.arg(0, None).unwrap_or_else(|| Value::data("data"));
            an.pyd_convert(&ann, &data, st, &c.node)
        }
        "validate_json" => {
            let text = c.arg(0, None).unwrap_or_else(|| Value::data("json"));
            let jc = Call { qual: "json.loads".into(), recv: None, args: vec![text], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: None, site: c.site.clone(), line: c.line };
            let data = crate::models::json_loads(an, st, &jc);
            an.pyd_convert(&ann, &data, st, &c.node)
        }
        "dump_python" => {
            let v = c.arg(0, None).unwrap_or_default();
            an.pyd_dump_value(&ann, &v, &DumpOpts::default(), st, &c.node)
        }
        _ => Value::data("TypeAdapter"),
    }
}

/// The ids of the models a `model_dump_json()` text holds.
pub fn json_models(v: &Value) -> Vec<ObjId> {
    let mut out = Vec::new();
    if let KeyVal::Data(s0) = &v.kv {
        let t = intern::str_get(*s0);
        if let Some(rest) = t.strip_prefix("pydjson:") {
            out.extend(rest.split(',').filter_map(|x| x.parse::<u32>().ok()));
        }
    }
    out
}

/// `json.loads(m.model_dump_json())`: the dump.
pub fn json_of_models(an: &mut Analyzer, st: &mut State, c: &Call, ms: &[ObjId]) -> Value {
    let opts = DumpOpts::default();
    join_values(ms.iter().map(|o| an.pyd_dump(*o, &opts, st, &c.node)).collect::<Vec<_>>())
}

pub fn install(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["pydantic.*"], m_instance);
    r(&["pydantic.cls.*"], m_class);
    r(&["pydantic.TypeAdapter", "pydantic.type_adapter.TypeAdapter"], m_type_adapter);
    r(&["pydantic.Field", "pydantic.fields.Field", "pydantic.PrivateAttr", "pydantic.ConfigDict", "pydantic_settings.SettingsConfigDict"], crate::models::noeffect);
}
