//! pydantic: a `BaseModel` subclass is a class with declared fields.
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
    /// (list / tuple / set / frozenset / sequence / iterable: the container's own errors)
    List(Box<Ann>, &'static str),
    Dict(Box<Ann>),
    Optional(Box<Ann>),
    /// `int` / `float` / `str` / `bool`: literals are coerced (lax mode)
    Scalar(&'static str),
    /// a `TypedDict` class: a dict of its declared keys (extras dropped)
    TypedDict(ObjId),
    /// a type variable of a generic model (`T`), bound by `Model[X]`
    Var(String),
    /// a union of models (`Cat | Dog`), with the discriminator field when given
    Union(Vec<Ann>, Option<String>),
    /// an Enum class: a member, or a value equal to one's
    Enum(ObjId),
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
    /// `Literal["cat", …]`: the values the field takes (a union's tags)
    pub lits: Option<Vec<KeyId>>,
    /// `Field(min_length=3, gt=0, pattern=…)` (also in `Annotated[…]`): name, literal value
    pub constraints: Vec<(String, KeyId)>,
}

#[derive(Clone, Debug)]
pub struct Validator {
    pub func: ObjId,
    /// the fields (`*`: all) of a field validator; empty for a model validator
    pub fields: Vec<String>,
    /// "before" / "after" / "wrap" / "plain"
    pub mode: String,
}

/// One problem a validation finds: where (the input key), pydantic's error type, whether
/// it surely happens, whether its record has a `ctx`.
#[derive(Clone, Debug, PartialEq)]
pub struct PErr {
    pub loc: Option<KeyId>,
    pub typ: &'static str,
    pub sure: bool,
    pub ctx: bool,
}

#[derive(Clone, Debug)]
pub struct PClass {
    pub fields: Vec<PField>,
    pub extra: Extra,
    pub populate_by_name: bool,
    /// `frozen=True`: assignment raises
    pub frozen: bool,
    /// `validate_assignment=True`: an assigned field value is validated
    pub validate_assignment: bool,
    /// a `BaseSettings`: (env_prefix, env_nested_delimiter)
    pub settings: Option<(String, Option<String>)>,
    pub root: bool,
    pub model_before: Vec<Validator>,
    pub model_after: Vec<Validator>,
    /// v1 `@root_validator` (post): gets the dict of field values, returns it
    pub root_post: Vec<Validator>,
    pub field_validators: Vec<Validator>,
    pub computed: Vec<String>,
    /// private attributes (`_x: T = PrivateAttr(default=…)` / `= value`): name, default, factory
    pub privates: Vec<(String, Option<&'static ast::Expr>, Option<&'static ast::Expr>)>,
    /// `@field_serializer("f")`: (self, value) → the dumped value
    pub field_serializers: Vec<Validator>,
    /// `@model_serializer`: (self) → the dump; `mode="wrap"`: (self, handler)
    pub model_serializer: Option<Validator>,
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

/// Is `cls` validated by pydantic (a model or a pydantic dataclass)?
pub fn is_validated(an: &Analyzer, cls: ObjId) -> bool {
    is_model(an, cls) || (an.class_info_opt(cls).is_some() && an.mro(cls).iter().any(|c| an.class_info_opt(*c).map(|x| x.pyd_dataclass.get()).unwrap_or(false)))
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

/// The exceptions pydantic turns into validation errors when a validator raises them.
const PYD_CAUGHT: &[&str] = &["ValueError", "AssertionError", "PydanticCustomError", "PydanticKnownError", "ValidationError"];

impl Analyzer {
    /// The pydantic description of model class `cls` (cached).
    pub fn pyd_class(&mut self, cls: ObjId, state: &State) -> Rc<PClass> {
        if let Some(p) = self.m.pyd_classes.get(&cls) {
            return p.clone();
        }
        let mut fields: Vec<PField> = Vec::new();
        let mut extra = Extra::Ignore;
        let mut populate_by_name = false;
        let mut root_ann: Option<Ann> = None;
        let mut settings: Option<(String, Option<String>)> = None;
        let mut frozen = false;
        let mut validate_assignment = false;
        let mut root = false;
        let (mut model_before, mut model_after, mut root_post, mut field_validators, mut computed) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut privates: Vec<(String, Option<&'static ast::Expr>, Option<&'static ast::Expr>)> = Vec::new();
        let mut field_serializers: Vec<Validator> = Vec::new();
        let mut model_serializer: Option<Validator> = None;
        // bases first: a subclass overrides fields, config and validators keep accumulating
        for c in self.mro(cls).into_iter().rev() {
            let Some(ci) = self.class_info_opt(c) else { continue };
            if !is_validated(self, c) {
                continue;
            }
            let cnode: &'static ast::StmtClassDef = ci.node;
            if let Some(args) = &cnode.arguments {
                if args.args.iter().any(|b| type_name(b) == "BaseSettings") && settings.is_none() {
                    settings = Some((String::new(), None));
                }
                for b in args.args.iter() {
                    if let ast::Expr::Subscript(sb) = b {
                        if type_name(&sb.value) == "RootModel" {
                            root = true;
                            let t: &'static ast::Expr = &sb.slice;
                            root_ann = Some(self.pyd_ann(t, ci.module, state));
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
                        Some("frozen") => frozen = bool_lit(&k.value).unwrap_or(frozen),
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
                                config_of(v, &mut extra, &mut populate_by_name, &mut frozen);
                                validate_assignment |= config_flag(v, "validate_assignment");
                                settings_of(v, &mut settings);
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
                        let mut f = PField { name: name.clone(), alias: None, ser_alias: None, alts: Vec::new(), default: None, factory: None, required: true, optional, ann, lits: literal_tags(&a.annotation), constraints: annotated_constraints(&a.annotation) };
                        if let Some(v) = &a.value {
                            let v: &'static ast::Expr = v;
                            match v {
                                // (`dataclasses.field(...)` in a pydantic dataclass too)
                                ast::Expr::Call(c) if matches!(dotted_last(&c.func).as_str(), "Field" | "field") => {
                                    f.constraints.extend(field_constraints(c));
                                    if let Some(d) = c.arguments.args.first().filter(|_| dotted_last(&c.func) == "Field") {
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
                                config_of(&a.value, &mut extra, &mut populate_by_name, &mut frozen);
                                validate_assignment |= config_flag(&a.value, "validate_assignment");
                                settings_of(&a.value, &mut settings);
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
                                        "frozen" => frozen = bool_lit(&a.value).unwrap_or(frozen),
                                        "validate_assignment" => validate_assignment = bool_lit(&a.value).unwrap_or(validate_assignment),
                                        "allow_mutation" => frozen = !bool_lit(&a.value).unwrap_or(!frozen),
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
                                "field_serializer" => {
                                    field_serializers.push(Validator { func, fields: args_s, mode: mode.unwrap_or_else(|| "plain".into()) });
                                }
                                "model_serializer" => {
                                    model_serializer = Some(Validator { func, fields: vec![], mode: mode.unwrap_or_else(|| "plain".into()) });
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
            fields.push(PField { name: "root".into(), alias: None, ser_alias: None, alts: Vec::new(), default: None, factory: None, required: true, optional: false, ann: root_ann.unwrap_or(Ann::Other), lits: None, constraints: Vec::new() });
        }
        if let Some(bind) = self.m.pyd_generic.get(&cls).cloned() {
            for f in fields.iter_mut() {
                f.ann = subst(&f.ann, &bind);
            }
        }
        let p = Rc::new(PClass { fields, extra, populate_by_name, frozen, validate_assignment, settings, root, model_before, model_after, root_post, field_validators, computed, privates, field_serializers, model_serializer });
        self.m.pyd_classes.insert(cls, p.clone());
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
                    "Annotated" => {
                        let inner = self.pyd_ann(elts[0], module, state);
                        // `Field(discriminator="kind")` on a union
                        let disc = elts.iter().skip(1).find_map(|m| match m {
                            ast::Expr::Call(c) if dotted_last(&c.func) == "Field" => kw(c, "discriminator").and_then(str_lit),
                            _ => None,
                        });
                        match (inner, disc) {
                            (Ann::Union(ms, _), Some(d)) => Ann::Union(ms, Some(d)),
                            (x, _) => x,
                        }
                    }
                    "list" | "List" | "Sequence" | "set" | "Set" | "tuple" | "Tuple" | "frozenset" | "FrozenSet" | "Iterable" => {
                        if base.to_lowercase().starts_with("tuple") && elts.len() != 1 && !(elts.len() == 2 && is_ellipsis(elts[1])) {
                            return Ann::Other;
                        }
                        let kind = match base.to_lowercase().as_str() {
                            "tuple" => "tuple",
                            "set" => "set",
                            "frozenset" => "frozenset",
                            "sequence" => "sequence",
                            "iterable" => "iterable",
                            _ => "list",
                        };
                        Ann::List(Box::new(self.pyd_ann(elts[0], module, state)), kind)
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
                            let ms: Vec<Ann> = non_none.iter().map(|x| self.pyd_ann(x, module, state)).collect();
                            let u = if ms.len() == 1 { ms.into_iter().next().unwrap() } else if ms.iter().all(|m| matches!(m, Ann::Model(_))) { Ann::Union(ms, None) } else { Ann::Other };
                            if non_none.len() < elts.len() {
                                Ann::Optional(Box::new(u))
                            } else {
                                u
                            }
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
                    _ => {
                        // `A | B | None`: the members, flattened
                        let mut ms: Vec<Ann> = Vec::new();
                        let mut none = false;
                        for x in [self.pyd_ann(l, module, state), self.pyd_ann(r, module, state)] {
                            match x {
                                Ann::Union(xs, _) => ms.extend(xs),
                                Ann::Optional(inner) => {
                                    none = true;
                                    match *inner {
                                        Ann::Union(xs, _) => ms.extend(xs),
                                        y => ms.push(y),
                                    }
                                }
                                y => ms.push(y),
                            }
                        }
                        let u = if ms.len() == 1 { ms.into_iter().next().unwrap() } else if ms.iter().all(|m| matches!(m, Ann::Model(_))) { Ann::Union(ms, None) } else { Ann::Other };
                        if none {
                            Ann::Optional(Box::new(u))
                        } else {
                            u
                        }
                    }
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
                let mut found = v.pts.iter().find(|o| self.kind(*o) == Kind::Class && is_validated(self, *o));
                if found.is_none() {
                    // by name among the program's classes (this module's first): the state at
                    // hand may be a summary's, without the module's names
                    let cands: Vec<ObjId> = self.user_classes.iter().copied().filter(|c| self.class_info_opt(*c).map(|ci| ci.name == n).unwrap_or(false) && is_validated(self, *c)).collect();
                    found = cands.iter().copied().find(|c| self.class_info(*c).module == module).or_else(|| cands.first().copied());
                }
                if found.is_none() {
                    let td = v.pts.iter().find(|o| self.kind(*o) == Kind::Class && self.class_info_opt(*o).map(|ci| ci.typed_dict).unwrap_or(false)).or_else(|| {
                        let c: Vec<ObjId> = self.user_classes.iter().copied().filter(|c| self.class_info_opt(*c).map(|ci| ci.name == n && ci.typed_dict).unwrap_or(false)).collect();
                        c.iter().copied().find(|x| self.class_info(*x).module == module).or_else(|| c.first().copied())
                    });
                    if let Some(t) = td {
                        return Ann::TypedDict(t);
                    }
                }
                if found.is_none() && self.m.pyd_ann_depth < 8 {
                    // a type alias: `Pet = Annotated[Union[Cat, Dog], …]` in the module
                    let tree = self.module_info(module).tree;
                    let alias = tree.body.iter().find_map(|st0| match st0 {
                        ast::Stmt::Assign(a) if matches!(&a.targets[..], [ast::Expr::Name(t)] if t.id.as_str() == n) => {
                            let v: &'static ast::Expr = &a.value;
                            match v {
                                ast::Expr::Subscript(_) | ast::Expr::BinOp(_) => Some(v),
                                _ => None,
                            }
                        }
                        ast::Stmt::TypeAlias(ta) if matches!(&*ta.name, ast::Expr::Name(t) if t.id.as_str() == n) => Some(&*ta.value),
                        _ => None,
                    });
                    if let Some(e2) = alias {
                        self.m.pyd_ann_depth += 1;
                        let a = self.pyd_ann(e2, module, state);
                        self.m.pyd_ann_depth -= 1;
                        return a;
                    }
                }
                if found.is_none() {
                    // a type variable: `T = TypeVar("T")` in the module
                    let tree = self.module_info(module).tree;
                    let is_var = tree.body.iter().any(|st0| match st0 {
                        ast::Stmt::Assign(a) => matches!(&a.targets[..], [ast::Expr::Name(t)] if t.id.as_str() == n) && matches!(&*a.value, ast::Expr::Call(c) if dotted_last(&c.func) == "TypeVar"),
                        _ => false,
                    });
                    if is_var {
                        return Ann::Var(n);
                    }
                }
                if found.is_none() {
                    let ec = v.pts.iter().find(|o| self.enum_members.contains_key(o)).or_else(|| {
                        let c: Vec<ObjId> = self.user_classes.iter().copied().filter(|c| self.enum_members.contains_key(c) && self.class_info_opt(*c).map(|ci| ci.name == n).unwrap_or(false)).collect();
                        c.iter().copied().find(|x| self.class_info(*x).module == module).or_else(|| c.first().copied())
                    });
                    if let Some(c) = ec {
                        return Ann::Enum(c);
                    }
                }
                match found {
                    Some(c) => Ann::Model(c),
                    None => match n.as_str() {
                        "int" => Ann::Scalar("int"),
                        "float" => Ann::Scalar("float"),
                        "str" => Ann::Scalar("str"),
                        "bool" => Ann::Scalar("bool"),
                        "Decimal" => Ann::Scalar("Decimal"),
                        "datetime" => Ann::Scalar("datetime"),
                        "date" => Ann::Scalar("date"),
                        "time" => Ann::Scalar("time"),
                        _ => Ann::Other,
                    },
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
        // (per call context: two constructions at one node from different callers stay apart)
        let ictx = "pydinput".to_string();
        let did = self.alloc_at(Kind::Dict, node, state, &format!("{}(…)@L{}", self.class_info(cls).name, node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&ictx));
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
        if let Some((prefix, delim)) = pc.settings.clone() {
            self.pyd_settings_env(&pc, &prefix, delim.as_deref(), did, state, node);
        }
        let out = self.pyd_validate(cls, &Value::obj(did), state, node, true);
        if self.m.pyd_last_failed {
            // whether it fails depends on a parameter's input: decided per call site
        }
        out
    }

    /// A settings class's input: each field not given as a keyword from the environment
    /// (`PREFIX` + name, case-insensitive; a nested model from `PREFIX_FIELD__SUB` keys).
    fn pyd_settings_env(&mut self, pc: &PClass, prefix: &str, delim: Option<&str>, did: ObjId, state: &mut State, node: &Node) {
        let Some(env) = crate::models::external_record(self, "os.environ", state) else { return };
        // (the environment as the call site has it)
        let er = state.row(env, Ns::Item);
        let up = |k: KeyId| intern::key_as_str(k).map(|x| x.to_uppercase());
        for f in pc.fields.iter() {
            let fk = key_str_lit(f.alias.as_deref().unwrap_or(&f.name));
            let given = state.row(did, Ns::Item);
            if given.definite.contains(fk) {
                continue;
            }
            let want = format!("{}{}", prefix, f.alias.as_deref().unwrap_or(&f.name)).to_uppercase();
            let mut v = Value::new();
            let mut sure = false;
            let mut maybe = false;
            for k in er.present.iter() {
                if up(k).as_deref() == Some(want.as_str()) {
                    v = v.join(&state.slot(env, Ns::Item, k));
                    maybe = true;
                    sure |= er.definite.contains(k);
                }
            }
            // a nested model: `PREFIX_DB__POOL` → {"pool": …}
            if let (Some(d), Ann::Model(_)) = (delim, &f.ann) {
                let pre = format!("{}{}", want, d.to_uppercase());
                let subs: Vec<(KeyId, KeyId, bool)> = er.present.iter().filter_map(|k| {
                    let u = up(k)?;
                    let rest = u.strip_prefix(&pre)?;
                    Some((k, key_str_lit(&rest.to_lowercase()), er.definite.contains(k)))
                }).collect();
                if !subs.is_empty() {
                    let nid = self.alloc_at(Kind::Dict, node, state, &format!("env@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("envnest{}", f.name)));
                    let mut row = Row::new();
                    for (k, sk, d0) in subs {
                        row = row.store(sk, d0);
                        state.set_slot(nid, Ns::Item, sk, state.slot(env, Ns::Item, k), true);
                    }
                    if er.open() {
                        row = row.data_store(crate::domain::Source::new("os.environ", crate::domain::SourceKind::External));
                    }
                    state.set_row(nid, Ns::Item, row);
                    v = v.join(&Value::obj(nid));
                    maybe = true;
                    sure = true;
                }
            }
            if !maybe && er.open() && !er.absent.iter().any(|k| up(k).as_deref() == Some(want.as_str())) {
                // a variable the program does not set: maybe there
                v = Value::data(&format!("env {}", want));
                maybe = true;
            }
            if !maybe {
                continue;
            }
            let given_maybe = given.present.contains(fk);
            let r = given.store(fk, sure || given.definite.contains(fk));
            state.set_row(did, Ns::Item, r);
            let cur = if given_maybe { state.slot(did, Ns::Item, fk) } else { Value::new() };
            state.set_slot(did, Ns::Item, fk, cur.join(&v), true);
        }
    }

    fn pyd_root_instance(&mut self, cls: ObjId, data: &Value, state: &mut State, node: &Node) -> Value {
        let ann = self.pyd_class(cls, state).fields.iter().find(|f| f.name == "root").map(|f| f.ann.clone()).unwrap_or(Ann::Other);
        let data = &self.pyd_convert(&ann, data, state, node);
        // one instance per input record (nested models of one class in one validation stay apart)
        let site = {
            let ins: Vec<ObjId> = data.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Dict | Kind::Unknown | Kind::Instance | Kind::Namespace)).collect();
            if ins.len() == 1 {
                format!("{}:{}:in{}", crate::interp::site_string(node), self.class_info(cls).name, ins[0])
            } else {
                format!("{}:{}", crate::interp::site_string(node), self.class_info(cls).name)
            }
        };
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
        // (inside the validation of another model: its errors go to that one's list)
        let nested = !self.m.pyd_errs.is_empty();
        let nest_prefix = self.m.pyd_nest_prefix.take();
        let saved_field = self.m.pyd_field;
        self.m.pyd_errs.push(Vec::new());
        // `mode="before"` model validators (v1 `root_validator(pre=True)`) see the raw input
        // and return what is validated
        for v in pc.model_before.clone() {
            let (out, sure, maybe) = self.pyd_catching(v.func, Value::obj(cls), vec![data.clone()], state, node);
            if maybe {
                self.pyd_err(None, "value_error", sure, true);
            }
            data = out;
        }
        // one instance per input record (nested models of one class in one validation stay apart)
        let site = {
            let ins: Vec<ObjId> = data.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Dict | Kind::Unknown | Kind::Instance | Kind::Namespace)).collect();
            if ins.len() == 1 {
                format!("{}:{}:in{}", crate::interp::site_string(node), self.class_info(cls).name, ins[0])
            } else {
                format!("{}:{}", crate::interp::site_string(node), self.class_info(cls).name)
            }
        };
        let inst = self.alloc(Kind::Instance, &site, state, &format!("{}@L{}", self.class_info(cls).name, node.line), Payload::Cls(cls), Some(Row::new()), Ns::Attr, None, None);
        state.set_row(inst, Ns::Attr, Row::new());
        let recs: Vec<ObjId> = data.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Dict | Kind::Unknown | Kind::Instance | Kind::Namespace)).collect();
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
            let mut dflt = Value::new();
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
                if f.required && !definite && (report || self.m.fa_body_validation) {
                    let sure_missing = !other_data && recs.iter().all(|o| !state.row(*o, ns_of(self, *o)).open() && !keys.iter().any(|k| state.row(*o, ns_of(self, *o)).present.contains(*k)));
                    self.pyd_err(Some(kk), "missing", sure_missing, false);
                }
                if f.required && definite && report {
                    // the field is there: the read succeeds
                    for &o in &recs {
                        if let Some(k) = keys.iter().find(|k| state.row(o, ns_of(self, o)).definite.contains(**k)) {
                            self.verdict(node, *k, &Classification::new(Tier::Proven), o, Ns::Item, None, None);
                        }
                    }
                }
                if f.required && !definite && keys.len() == 1 && report && recs.len() > 1 {
                    // several records (a validator returning different ones on different
                    // paths): each read on its own branch, so the input's requirement is
                    // conditional
                    let mut branches: Vec<State> = Vec::new();
                    for &o in &recs {
                        let mut b = state.copy();
                        self.branch_depth += 1;
                        let x = self.load_item_or_attr(&Value::obj(o), ns_of(self, o), kk, node, &mut b);
                        self.branch_depth -= 1;
                        val = val.join(&x);
                        branches.push(b);
                    }
                    let mut joined = branches.remove(0);
                    for mut b in branches {
                        joined = joined.join(&mut b);
                    }
                    state.replace_with(joined);
                } else if f.required && !definite && keys.len() == 1 && report {
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
                                self.verdict(node, kk, &Classification::with_note(Tier::Missing, "is a required field that was not given"), o, Ns::Item, None, None);
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
                        dflt = dflt.join(&self.pyd_default(&f, node, state));
                    } else if val.empty() {
                        dflt = self.pyd_default(&f, node, state);
                    }
                }
                let _ = present;
            } else if !f.required {
                dflt = self.pyd_default(&f, node, state);
            }
            if other_data && !f.required {
                dflt = dflt.join(&self.pyd_default(&f, node, state));
            }
            if val.empty() && dflt.empty() {
                val = Value::data(&format!("{}.{}", self.class_info(cls).name, f.name));
            }
            self.m.pyd_field = Some(kk);
            let saved_inst = self.m.pyd_inst.replace(inst);
            // a field surely missing / surely not converting: its validators do not run
            let under = |l: Option<KeyId>| l == Some(kk) || l.map(|l| intern::with_key(l, |x| matches!(x, intern::Key::Tuple(t) if t.first() == Some(&kk)))).unwrap_or(false);
            let failed = |an: &Analyzer| an.m.pyd_errs.last().map(|es| es.iter().any(|e| e.sure && under(e.loc))).unwrap_or(false);
            // field validators before the type's conversion, then the conversion, then after
            // (a default is not validated: validate_default is off)
            let given = !val.empty();
            if !failed(self) && given {
                for v in pc.field_validators.iter().filter(|v| (v.fields.contains(&f.name) || v.fields.iter().any(|x| x == "*")) && matches!(v.mode.as_str(), "before" | "wrap" | "plain")).cloned().collect::<Vec<_>>() {
                    val = self.pyd_call_validator(cls, &v, &val, inst, &f.name, state, node);
                }
                val = self.pyd_convert(&f.ann, &val, state, node);
                if let Some(ls) = f.lits.clone() {
                    val = self.pyd_check_literal(&ls, &val);
                }
                if !f.constraints.is_empty() && !failed(self) {
                    self.pyd_check_constraints(&f.constraints, &val, state);
                }
            }
            if !failed(self) && given {
                for v in pc.field_validators.iter().filter(|v| (v.fields.contains(&f.name) || v.fields.iter().any(|x| x == "*")) && v.mode == "after").cloned().collect::<Vec<_>>() {
                    val = self.pyd_call_validator(cls, &v, &val, inst, &f.name, state, node);
                }
            }
            val = val.join(&dflt);
            self.m.pyd_inst = saved_inst;
            let r = state.row(inst, Ns::Attr).store(nk, true);
            state.set_row(inst, Ns::Attr, r);
            state.set_slot(inst, Ns::Attr, nk, val, true);
        }
        let entry = (KeySet::from_iter(set_sure), KeySet::from_iter(set_maybe));
        let joined = match self.m.pyd_set.get(&inst) {
            Some((s0, m0)) if !self.obj(inst).single => (s0.intersection(&entry.0), m0.union(&entry.1)),
            _ => entry,
        };
        self.m.pyd_set.insert(inst, joined);
        self.m.pyd_field = None;
        // extra keys with `extra="forbid"`: an error per key
        if pc.extra == Extra::Forbid && report {
            let fk: KeySet = KeySet::from_iter(field_keys.iter().copied());
            for &o in &recs {
                let ns = if matches!(self.kind(o), Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
                let row = state.row(o, ns);
                for k in row.present.iter() {
                    if !fk.contains(k) {
                        let sure = recs.len() == 1 && row.definite.contains(k);
                        self.pyd_err(Some(k), "extra_forbidden", sure, false);
                    }
                }
                if row.open() {
                    self.pyd_err(None, "extra_forbidden", false, false);
                }
            }
        }
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
        // v1 `@root_validator` (post): the dict of field values in (the values themselves, not
        // their dumps), a dict out whose values become the fields
        for v in pc.root_post.clone() {
            let did = self.alloc_at(Kind::Dict, node, state, &format!("values@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("rootv{}", inst)));
            let mut vrow = Row::new();
            let irow = state.row(inst, Ns::Attr);
            for f in pc.fields.iter() {
                let nk = key_str_lit(&f.name);
                if irow.present.contains(nk) {
                    vrow = vrow.store(nk, irow.definite.contains(nk));
                    state.set_slot(did, Ns::Item, nk, state.slot(inst, Ns::Attr, nk), true);
                }
            }
            state.set_row(did, Ns::Item, vrow);
            let out = self.call_user(v.func, Some(Value::obj(cls)), vec![Value::obj(did)], IndexMap::new(), vec![], vec![], state, node, None, false);
            let outs: Vec<ObjId> = out.pts.iter().filter(|o| self.kind(*o) == Kind::Dict).collect();
            let strong = outs.len() == 1 && out.kv.is_bot();
            for d in outs {
                for f in pc.fields.iter() {
                    let nk = key_str_lit(&f.name);
                    if state.row(d, Ns::Item).present.contains(nk) {
                        let nv = state.slot(d, Ns::Item, nk);
                        state.set_slot(inst, Ns::Attr, nk, nv, strong && state.row(d, Ns::Item).definite.contains(nk));
                    }
                }
            }
        }
        self.pyd_init_privates(cls, inst, state, node);
        // (`mode="after"` model validators run only when the fields validated: not after a
        // sure field error; after a maybe one, their own error is a maybe one)
        let field_failed = self.m.pyd_errs.last().map(|es| es.iter().any(|e| e.sure)).unwrap_or(false);
        let field_maybe = self.m.pyd_errs.last().map(|es| !es.is_empty()).unwrap_or(false);
        for v in pc.model_after.clone() {
            if field_failed {
                break;
            }
            let (_, sure, maybe) = self.pyd_catching(v.func, Value::obj(inst), vec![], state, node);
            if maybe {
                self.pyd_err(None, "value_error", sure && !field_maybe, true);
            }
        }
        // the problems found: a ValidationError (surely, or on some paths)
        let errs = self.m.pyd_errs.pop().unwrap_or_default();
        self.m.pyd_last_failed = !errs.is_empty();
        if !errs.is_empty() {
            // whether it fails depends on a parameter's input: decided per call site
        }
        if nested {
            // the enclosing validation reports them: loc under the field (unknown when the
            // nested model is not a plain field: an item of a list, a dict value)
            self.m.pyd_field = saved_field;
            for e in errs {
                let loc = match (nest_prefix, e.loc) {
                    (Some(p), Some(l)) => {
                        let mut parts = vec![p];
                        match intern::with_key(l, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None }) {
                            Some(t) => parts.extend(t),
                            None => parts.push(l),
                        }
                        Some(intern::key_tuple(parts))
                    }
                    (Some(p), None) => Some(intern::key_tuple(vec![p])),
                    (None, _) => None,
                };
                if let Some(top) = self.m.pyd_errs.last_mut() {
                    top.push(PErr { loc, typ: e.typ, sure: e.sure, ctx: e.ctx });
                }
            }
            outs.push(Value::obj(inst));
            return join_values(outs);
        }
        if !report && self.m.fa_body_validation && !errs.is_empty() && state.reachable {
            self.pyd_raise_errors_as(errs.clone(), state, node, "RequestValidationError");
        }
        if report && !errs.is_empty() && state.reachable {
            self.pyd_raise_errors(errs, state, node);
        }
        outs.push(Value::obj(inst));
        join_values(outs)
    }

    /// A ValidationError holding `errs` (its `errors()`), raised here when one is sure.
    fn pyd_raise_errors(&mut self, errs: Vec<PErr>, state: &mut State, node: &Node) {
        self.pyd_raise_errors_as(errs, state, node, "ValidationError");
    }

    /// The same, raised as `cls` (FastAPI's `RequestValidationError` for a request body: the
    /// errors object travels with it, for the 422 detail).
    fn pyd_raise_errors_as(&mut self, errs: Vec<PErr>, state: &mut State, node: &Node, cls: &str) {
        let sure = errs.iter().any(|e| e.sure);
        // (an equal list made before: the same one)
        let idx = match self.m.pyd_verrs.iter().position(|l| *l == errs) {
            Some(i) => i,
            None => {
                self.m.pyd_verrs.push(errs);
                self.m.pyd_verrs.len() - 1
            }
        };
        // (per call context: separate calls, unrolled rounds keep their own errors)
        let ctx = { self.frame().ctx.clone() };
        let eo = self.alloc_raw(Kind::Namespace, &format!("pydverr:{}", crate::interp::site_string(node)), &ctx, false, "ValidationError", Payload::Method(format!("pydverr.{}", idx), Value::new()));
        // (one object per raise site: a second raise there — another loop round, another
        // record — makes it either one's errors, in no known order)
        let mut use_idx = idx;
        if let Payload::Method(q, _) = &self.obj(eo).payload {
            if let Some(old) = q.strip_prefix("pydverr.").and_then(|x| x.parse::<usize>().ok()).filter(|o| *o != idx) {
                let mut both: Vec<PErr> = self.m.pyd_verrs.get(old).cloned().unwrap_or_default();
                both.extend(self.m.pyd_verrs[idx].iter().cloned());
                for e in both.iter_mut() {
                    e.sure = false;
                }
                // (the same error twice: once)
                let mut uniq: Vec<PErr> = Vec::new();
                for e in both {
                    if !uniq.iter().any(|u| u.loc == e.loc && u.typ == e.typ && u.ctx == e.ctx) {
                        uniq.push(e);
                    }
                }
                // (an equal list made before: the same one, so a loop's rounds converge)
                use_idx = match self.m.pyd_verrs.iter().position(|l| *l == uniq) {
                    Some(i) => i,
                    None => {
                        self.m.pyd_verrs.push(uniq);
                        self.m.pyd_verrs.len() - 1
                    }
                };
            }
        }
        self.obj_mut(eo).payload = Payload::Method(format!("pydverr.{}", use_idx), Value::new());
        state.set_row(eo, Ns::Attr, Row::new().data_store(crate::domain::Source::new("ValidationError", crate::domain::SourceKind::Data)));
        self.cur_exc = Some(Value::obj(eo));
        if cls != "ValidationError" {
            self.cur_exc_cls = Some(crate::interp::builtin_exc_names(cls));
        }
        if sure {
            self.raise_here(cls, state);
        } else {
            self.may_raise_here(cls, state);
        }
        self.cur_exc = None;
        self.cur_exc_cls = None;
    }

    /// Field constraints on known values: an error when every value breaks one (maybe when
    /// some do).
    fn pyd_check_constraints(&mut self, cons: &[(String, KeyId)], val: &Value, state: &State) {
        let lits: Vec<KeyId> = crate::domain::kv_literals(&val.kv).iter().collect();
        let mut n = lits.len() + val.pts.len();
        if val.kv.is_bot() && val.pts.is_empty() {
            return;
        }
        if !val.kv.is_literal() && !val.kv.is_bot() {
            // unknown values may break them
            n += 1;
        }
        let mut errs: Vec<(&'static str, usize)> = Vec::new();
        let note = |code: &'static str, errs: &mut Vec<(&'static str, usize)>| match errs.iter_mut().find(|(c, _)| *c == code) {
            Some(e) => e.1 += 1,
            None => errs.push((code, 1)),
        };
        for k in &lits {
            if *k == intern::key_none() {
                continue;
            }
            if let Some(code) = literal_breaks(cons, *k) {
                note(code, &mut errs);
            }
        }
        // lists / sets / dicts with a known size
        for o in val.pts.iter() {
            let size = match self.kind(o) {
                Kind::List | Kind::Tuple | Kind::Set => self.llen(o, state),
                Kind::Dict => {
                    let r = state.row(o, Ns::Item);
                    (r.closed() && r.present == r.definite).then(|| r.present.len())
                }
                _ => None,
            };
            let Some(sz) = size else { continue };
            for (c, lim) in cons {
                let Some(m) = intern::key_as_int(*lim) else { continue };
                let code = match c.as_str() {
                    "min_length" if (sz as i64) < m => "too_short",
                    "max_length" if (sz as i64) > m => "too_long",
                    _ => continue,
                };
                note(code, &mut errs);
                break;
            }
        }
        let loc = self.m.pyd_field;
        for (code, cnt) in errs {
            self.pyd_err(loc, code, cnt == n, true);
        }
    }

    fn pyd_err(&mut self, loc: Option<KeyId>, typ: &'static str, sure: bool, ctx: bool) {
        if let Some(top) = self.m.pyd_errs.last_mut() {
            top.push(PErr { loc, typ, sure, ctx });
        }
    }

    /// A validator called the way pydantic calls it: a ValueError / AssertionError it raises
    /// is a validation error (collected; validation goes on); others escape. Returns the
    /// value and whether the validator surely / maybe raised such an error.
    fn pyd_catching(&mut self, f: ObjId, self_v: Value, args: Vec<Value>, state: &mut State, node: &Node) -> (Value, bool, bool) {
        let mut pre = state.copy();
        self.enter_try(rustc_hash::FxHashSet::from_iter(PYD_CAUGHT.iter().map(|x| x.to_string())), &mut pre);
        // (run with the validated values: `v.strip()` of a str field is a str method there)
        let out = self.call_user(f, Some(self_v), args, IndexMap::new(), vec![], vec![], state, node, None, false);
        // an exception of another class (a KeyError, one of code rowan cannot see) is no
        // validation error: it escapes to the enclosing try / call
        let validation = |c: &Option<crate::interp::ExcNames>| c.as_ref().is_some_and(|(n, _)| n.iter().any(|x| PYD_CAUGHT.contains(&x.as_str())));
        let (recs, escaping): (Vec<_>, Vec<_>) = self.leave_try().recs.into_iter().partition(|r| validation(&r.0));
        for r in escaping {
            self.reraise_rec(r);
        }
        let raised_at = crate::interp::join_raise_recs(&recs);
        let Some(mut r) = raised_at else { return (out, false, false) };
        let died = !state.reachable;
        let after = if state.reachable { state.copy().join(&mut r) } else { r };
        state.replace_with(after);
        (out.join(&Value::data("validator")), died, true)
    }

    /// A converted list (`List[M]`) at `node` in context `ctx`: positions when known.
    fn pyd_list(&mut self, vals: Option<Vec<Value>>, elem: Value, node: &Node, state: &mut State, ctx: &str) -> Value {
        let nid = self.alloc_at(Kind::List, node, state, &format!("list@L{}", node.line), Payload::None, None, Ns::Item, None, Some(ctx));
        let single = self.obj(nid).single;
        let acc = match &vals {
            Some(vs) => join_values(vs.clone()),
            None => elem,
        };
        state.set_slot(nid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), single);
        state.elems.insert(nid, crate::domain::DATA);
        match vals {
            Some(vs) if single => {
                self.set_llen(nid, state, Some(vs.len()));
                for (i, v) in vs.iter().enumerate() {
                    state.set_slot(nid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
                }
            }
            _ => self.set_llen(nid, state, None),
        }
        Value::obj(nid)
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

    /// An assignment `o.name = v` on a model instance / typed placeholder: Some(value to
    /// store) — validated when `validate_assignment` — or None when it raises (a non-field
    /// on a model without `extra="allow"`: ValueError).
    pub fn pyd_assign(&mut self, oid: ObjId, cls: ObjId, name: &str, v: &Value, node: &Node, state: &mut State) -> Option<Value> {
        let pc = self.pyd_class(cls, state);
        let nk = key_str_lit(name);
        match pc.fields.iter().find(|f| f.name == name) {
            Some(f) if pc.validate_assignment => {
                let ann = f.ann.clone();
                Some(self.pyd_convert(&ann, v, state, node))
            }
            Some(_) => Some(v.clone()),
            None if self.class_lookup(cls, name).is_some() => Some(v.clone()),
            None if pc.extra == Extra::Allow => {
                self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                Some(v.clone())
            }
            None => {
                self.verdict(node, nk, &Classification::with_note(Tier::Missing, "is not a field of the model"), oid, Ns::Attr, None, None);
                self.raise_here("ValueError", state);
                None
            }
        }
    }

    /// Is `name` a private attribute of model class `cls`?
    pub fn pyd_is_private(&mut self, cls: ObjId, name: &str, state: &State) -> bool {
        name.starts_with('_') && !name.starts_with("__") && is_model(self, cls) && self.pyd_class(cls, state).privates.iter().any(|p| p.0 == name)
    }

    /// `Model[X, …]` of a generic model: a class of its own (the model's body, its type
    /// variables bound to the arguments, in the order of `Generic[T, …]`).
    pub fn pyd_specialise(&mut self, cls: ObjId, slice: &'static ast::Expr, state: &mut State) -> Option<Value> {
        let ci = self.class_info(cls);
        let params: Vec<String> = ci.node.arguments.as_ref().map(|a| {
            a.args.iter().filter_map(|b| match b {
                ast::Expr::Subscript(sb) if matches!(dotted_last(&sb.value).as_str(), "Generic" | "BaseModel") => Some(match &*sb.slice {
                    ast::Expr::Tuple(t) => t.elts.iter().map(type_name).collect::<Vec<_>>(),
                    other => vec![type_name(other)],
                }),
                _ => None,
            }).flatten().collect()
        }).unwrap_or_default();
        if params.is_empty() {
            return None;
        }
        let elts: Vec<&'static ast::Expr> = match slice {
            ast::Expr::Tuple(t) => t.elts.iter().collect(),
            other => vec![other],
        };
        let module = self.frame().module;
        let mut bind: Vec<(String, Ann)> = Vec::new();
        let mut key = String::new();
        for (p, e) in params.iter().zip(elts.iter()) {
            // a name holding a class (`Envelope[cls]`): that class; else the annotation
            let v = self.eval(e, state);
            let a = match v.pts.iter().find(|o| self.kind(*o) == Kind::Class) {
                Some(c) if is_validated(self, c) => Ann::Model(c),
                Some(c) if self.class_info_opt(c).map(|x| x.typed_dict).unwrap_or(false) => Ann::TypedDict(c),
                _ => self.pyd_ann(e, module, state),
            };
            key.push_str(&format!("{:?};", a));
            bind.push((p.clone(), a));
        }
        let site = format!("generic:{}:{}", cls, key);
        let label = format!("{}[…]", ci.name);
        let info = std::rc::Rc::new(crate::interp::ClassInfo {
            name: ci.name.clone(),
            node: ci.node,
            bases: vec![cls],
            module: ci.module,
            open_base: false,
            dataclass_fields: std::cell::RefCell::new(Some(Vec::new())),
            typed_dict: false,
            named_tuple: false,
            dict_base: false,
            pydantic: ci.pydantic,
            pyd_dataclass: std::cell::Cell::new(ci.pyd_dataclass.get()),
            dc_frozen: std::cell::Cell::new(ci.dc_frozen.get()),
            dc_kw_only: std::cell::Cell::new(ci.dc_kw_only.get()),
            dc_order: std::cell::Cell::new(ci.dc_order.get()),
            nn_module: false,
            any_fields: std::cell::RefCell::new(None),
        });
        let spec = self.alloc_raw(Kind::Class, &site, "", true, &label, Payload::Class(info));
        self.m.pyd_generic.insert(spec, bind);
        self.m.pyd_classes.remove(&spec);
        Some(Value::obj(spec))
    }

    /// The keys of a `TypedDict` class: (name, required, type), bases first; `total=False`
    /// and `NotRequired[...]` make a key optional, `Required[...]` required.
    pub fn typed_dict_fields(&mut self, cls: ObjId, state: &State) -> Vec<(String, bool, Ann)> {
        let mut out: Vec<(String, bool, Ann)> = Vec::new();
        for c in self.mro(cls).into_iter().rev() {
            let Some(ci) = self.class_info_opt(c) else { continue };
            if !ci.typed_dict {
                continue;
            }
            let node: &'static ast::StmtClassDef = ci.node;
            let total = node.arguments.as_ref().and_then(|a| a.keywords.iter().find(|k| k.arg.as_ref().map(|x| x.as_str() == "total").unwrap_or(false))).and_then(|k| bool_lit(&k.value)).unwrap_or(true);
            for st in &node.body {
                let ast::Stmt::AnnAssign(a) = st else { continue };
                let ast::Expr::Name(n) = &*a.target else { continue };
                let (req, inner): (bool, &'static ast::Expr) = match &*a.annotation {
                    ast::Expr::Subscript(sb) if dotted_last(&sb.value) == "NotRequired" => (false, &sb.slice),
                    ast::Expr::Subscript(sb) if dotted_last(&sb.value) == "Required" => (true, &sb.slice),
                    other => (total, other),
                };
                let ann = self.pyd_ann(inner, ci.module, state);
                out.retain(|x| x.0 != n.id.as_str());
                out.push((n.id.to_string(), req, ann));
            }
        }
        out
    }

    /// A required field's read: `d[k]` on a dict / input, `o.k` on an object (verdicts and
    /// requirements as for the program's own reads).
    fn load_item_or_attr(&mut self, recv: &Value, ns: Ns, k: KeyId, node: &Node, state: &mut State) -> Value {
        // a ValidationError the program (or FastAPI: a 422) handles: no diagnostic
        let saved = self.verdict_guarded;
        self.verdict_guarded |= self.validation_caught();
        // (a missing field is pydantic's ValidationError, not a KeyError)
        let saved_exc = std::mem::replace(&mut self.item_exc_force, Some("ValidationError"));
        let v = if ns == Ns::Attr { self.load_attr(recv, &intern::key_py_str(k), node, state, true) } else { self.load_item(recv, Ns::Item, &Value::lit(k), node, state, true) };
        self.item_exc_force = saved_exc;
        self.verdict_guarded = saved;
        v
    }

    /// A try around here catches a ValidationError.
    pub fn validation_caught(&self) -> bool {
        self.guards().any(|g| g.iter().any(|x| matches!(x.as_str(), "ValidationError" | "ValueError" | "Exception" | "BaseException" | "*")))
    }

    /// A guarded read of `k` from the records (no verdict, no requirement): the values there.
    fn pyd_read_guarded(&mut self, recs: &[ObjId], k: KeyId, state: &mut State) -> Value {
        let mut out = Value::new();
        for &o in recs {
            let ns = if matches!(self.kind(o), Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
            let row = state.row(o, ns);
            if row.present.contains(k) {
                out = out.join(&state.slot(o, ns, k));
                // (a store under an unknown key may have written it)
                if ns == Ns::Item {
                    if let Some(sv) = state.slot_opt(o, ns, intern::STAR) {
                        out = out.join(&sv);
                    }
                }
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
            // (in the module defining the field: `Priority.NORMAL` names the model's module's;
            // a mutable default is copied per instance, here per construction site)
            Some(d) => {
                let v = self.pyd_default_expr(d, state);
                let mut out = Value::kv_only(v.kv.clone());
                for o in v.pts.iter() {
                    if matches!(self.kind(o), Kind::List | Kind::Dict | Kind::Set) {
                        let mut seen = IndexMap::new();
                        out = out.join(&Value::obj(self.copy_object(o, node, state, true, &mut seen)));
                    } else {
                        out = out.join(&Value::obj(o));
                    }
                }
                out
            }
            None => Value::new(),
        }
    }

    /// A `Literal[...]` field: a literal of an option's type is one of the options or an
    /// error; an unknown value is one of the options (or an error); a value of another type
    /// (`1.0` for `Literal[1]`) is not decided.
    fn pyd_check_literal(&mut self, ls: &[KeyId], val: &Value) -> Value {
        let kind = |k: KeyId| -> u8 {
            match intern::key_get(k) {
                intern::Key::Str(_) => 1,
                intern::Key::Int(_) => 2,
                intern::Key::Bool(_) => 3,
                intern::Key::None => 4,
                _ => 0,
            }
        };
        let opts = Value::kv_only(KeyVal::lits(KeySet::from_iter(ls.iter().copied())));
        let mut out = Value::new();
        let (mut bad, mut total, mut unsure) = (0, 0, false);
        for o in val.pts.iter() {
            total += 1;
            unsure = true;
            // (an unknown value, or a parameter's: one of the options; a call site that knows
            // it decides)
            match self.kind(o) {
                Kind::Unknown => out = out.join(&opts),
                _ => out = out.join(&Value::obj(o)),
            }
        }
        if val.kv.is_literal() {
            for k in crate::domain::kv_literals(&val.kv).iter() {
                total += 1;
                if ls.contains(&k) {
                    out = out.join(&Value::lit(k));
                } else if kind(k) != 0 && ls.iter().any(|l| kind(*l) == kind(k)) && ls.iter().all(|l| kind(*l) != 0) {
                    bad += 1;
                } else {
                    unsure = true;
                    out = out.join(&Value::lit(k));
                }
            }
        } else if !val.kv.is_bot() {
            total += 1;
            unsure = true;
            out = out.join(&opts);
        }
        if bad > 0 || unsure {
            let loc = self.m.pyd_field;
            self.pyd_err(loc, "literal_error", bad == total && bad > 0, false);
        }
        out
    }

    /// A field's default expression, evaluated in the module defining it.
    fn pyd_default_expr(&mut self, d: &'static ast::Expr, state: &mut State) -> Value {
        let here = self.module_info(self.frame().module).idx;
        let owner = crate::frontend::module_of_addr(d as *const ast::Expr as usize);
        if let Some(idx) = owner.filter(|i| *i != here) {
            if let Some(moid) = self.modules.values().copied().find(|m| self.module_info(*m).idx == idx) {
                return self.eval_in_module(moid, d, state);
            }
        }
        self.eval(d, state)
    }

    #[allow(clippy::too_many_arguments)]
    fn pyd_call_validator(&mut self, cls: ObjId, v: &Validator, val: &Value, inst: ObjId, field: &str, state: &mut State, node: &Node) -> Value {
        // field validators are classmethods: (cls, value[, info]); wrap: (cls, value, handler[, info])
        let nparams = match &self.func_info(v.func).node {
            crate::interp::FuncNode::Def(f) => crate::scopes::all_params(&f.parameters).len(),
            _ => 2,
        };
        // (an after validator of a scalar field gets the validated value: a `str` / `int` /
        // `float` / `bool` of the input, whatever placeholder it was read from)
        let val = val.clone();
        let val = &val;
        let mut args = vec![val.clone()];
        if v.mode == "wrap" {
            // the handler validates by the field's type
            let pc = self.pyd_class(cls, state);
            let ann = pc.fields.iter().find(|f| f.name == field).map(|f| f.ann.clone()).unwrap_or(Ann::Other);
            let idx = self.m.pyd_adapters.len();
            self.m.pyd_adapters.push(ann);
            let h = self.alloc_raw(Kind::Method, &format!("pydhandler:{}:{}", cls, field), "", true, "validator handler", Payload::Method(format!("typeadapter.{}.validate_python", idx), Value::new()));
            args.push(Value::obj(h));
        }
        if nparams > args.len() + 1 {
            // ValidationInfo: `data` holds the fields validated before this one (each may have
            // failed: present, not definite)
            let did = self.alloc_at(Kind::Dict, node, state, &format!("info.data@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("vinfo{}:{}", cls, field)));
            let ir = state.row(inst, Ns::Attr);
            let mut row = Row::new();
            for k in ir.present.iter() {
                row = row.store(k, false);
                state.set_slot(did, Ns::Item, k, state.slot(inst, Ns::Attr, k), true);
            }
            state.set_row(did, Ns::Item, row);
            let info = self.alloc_at(Kind::Namespace, node, state, "ValidationInfo", Payload::None, Some(Row::new()), Ns::Attr, None, Some(&format!("vinfo{}:{}:ns", cls, field)));
            let mut irow = Row::new();
            for (k, x) in [("data", Value::obj(did)), ("field_name", Value::lit_str(field)), ("mode", Value::lit_str("python")), ("context", self.none_v.clone())] {
                let kk = key_str_lit(k);
                irow = irow.store(kk, true);
                state.set_slot(info, Ns::Attr, kk, x, true);
            }
            state.set_row(info, Ns::Attr, irow);
            args.push(Value::obj(info));
        }
        let (out, sure, maybe) = self.pyd_catching(v.func, Value::obj(cls), args, state, node);
        if maybe {
            let loc = self.m.pyd_field;
            self.pyd_err(loc, "value_error", sure, true);
        }
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
                // (its errors are this model's, under this field)
                let saved_prefix = std::mem::replace(&mut self.m.pyd_nest_prefix, self.m.pyd_field);
                let mut out = self.pyd_validate(c, &Value::objs(crate::domain::ObjSet::from_vec(recs)), state, node, false);
                self.m.pyd_nest_prefix = saved_prefix;
                if !val.kv.is_bot() && val.kv.is_literal() {
                    out = out.join(&lits);
                }
                out
            }
            Ann::Optional(inner) => {
                // None passes; the rest is converted
                let none = intern::key_none();
                let has_none = val.kv.as_lit() == Some(none) || matches!(&val.kv, KeyVal::Lits(ks) if ks.contains(none));
                if !has_none {
                    return self.pyd_convert(inner, val, state, node);
                }
                let rest_kv = match &val.kv {
                    KeyVal::Lits(ks) => KeyVal::lits(ks.remove(none)),
                    _ => KeyVal::Bot,
                };
                let rest = Value::with_kv(val.pts.clone(), rest_kv);
                let conv = if rest.empty() { rest } else { self.pyd_convert(inner, &rest, state, node) };
                conv.join(&self.none_v.clone())
            }
            Ann::Union(ms, disc) => {
                let models: Vec<ObjId> = ms.iter().filter_map(|m| if let Ann::Model(c) = m { Some(*c) } else { None }).collect();
                let mut outs: Vec<Value> = Vec::new();
                if !val.kv.is_bot() {
                    outs.push(Value::kv_only(val.kv.clone()));
                }
                for o in val.pts.iter() {
                    // an instance of a member stays
                    if self.kind(o) == Kind::Instance && self.instance_class(o).map(|c| models.iter().any(|m| self.mro(c).contains(m))).unwrap_or(false) {
                        outs.push(Value::obj(o));
                        continue;
                    }
                    if !matches!(self.kind(o), Kind::Dict | Kind::Unknown) {
                        outs.push(Value::obj(o));
                        continue;
                    }
                    let mut chosen: Vec<ObjId> = models.clone();
                    if let Some(d) = disc {
                        // the tag: a required read; it picks the member
                        let dk = key_str_lit(d);
                        let tag = self.load_item(&Value::obj(o), Ns::Item, &Value::lit(dk), node, state, true);
                        if tag.pts.is_empty() && tag.kv.is_literal() {
                            let tl = crate::domain::kv_literals(&tag.kv);
                            chosen = models.iter().copied().filter(|m| {
                                let pc = self.pyd_class(*m, state);
                                pc.fields.iter().find(|f| f.name == *d).and_then(|f| f.lits.clone()).map(|ls| ls.iter().any(|l| tl.contains(*l))).unwrap_or(true)
                            }).collect();
                            if chosen.is_empty() {
                                // no member has that tag: a ValidationError (union_tag_invalid)
                                if self.m.pyd_errs.is_empty() {
                                    let errs = vec![PErr { loc: Some(dk), typ: "union_tag_invalid", sure: val.pts.len() == 1 && val.kv.is_bot(), ctx: false }];
                                    self.pyd_raise_errors(errs, state, node);
                                    if !state.reachable {
                                        return Value::new();
                                    }
                                } else if !self.m.pyd_errs.is_empty() {
                                    let loc = self.m.pyd_field.or(Some(dk));
                                    self.pyd_err(loc, "union_tag_invalid", val.pts.len() == 1 && val.kv.is_bot(), false);
                                } else {
                                    self.may_raise_here("ValidationError", state);
                                }
                                continue;
                            }
                        }
                    }
                    if chosen.len() == 1 {
                        outs.push(self.pyd_validate_opts(chosen[0], &Value::obj(o), state, node, true));
                        continue;
                    }
                    // each member on its own branch (the input's requirements per member)
                    let mut branches: Vec<State> = Vec::new();
                    for m in chosen {
                        let mut b = state.copy();
                        self.branch_depth += 1;
                        let x = self.pyd_validate_opts(m, &Value::obj(o), &mut b, node, true);
                        self.branch_depth -= 1;
                        outs.push(x);
                        if b.reachable {
                            branches.push(b);
                        }
                    }
                    if !branches.is_empty() {
                        let mut joined = branches.remove(0);
                        for mut b in branches {
                            joined = joined.join(&mut b);
                        }
                        state.replace_with(joined);
                    }
                }
                join_values(outs)
            }
            Ann::Scalar(t) if matches!(*t, "datetime" | "date" | "time") => {
                // ISO text parses; a datetime value passes; other inputs (timestamps …) are a
                // value of the type; text that cannot be a date fails
                let t = *t;
                use crate::models::chrono;
                let kind = match t {
                    "datetime" => chrono::DATETIME,
                    "date" => chrono::DATE,
                    _ => chrono::TIME,
                };
                if !(val.pts.is_empty() && val.kv.is_literal()) {
                    return Value::data(t);
                }
                let mut outs: Vec<Value> = Vec::new();
                let mut bad = 0;
                let lits = crate::domain::kv_literals(&val.kv);
                for k in lits.iter() {
                    if let Some((kk, v, tz)) = chrono::as_chrono(k) {
                        let conv = match (kind, kk) {
                            (a, b) if a == b => Some(k),
                            (chrono::DATE, chrono::DATETIME) if v.rem_euclid(86_400_000_000) == 0 => Some(chrono::chrono_key(chrono::DATE, v.div_euclid(86_400_000_000) + 1, chrono::NAIVE)),
                            (chrono::DATETIME, chrono::DATE) => Some(chrono::chrono_key(chrono::DATETIME, (v - 1) * 86_400_000_000, chrono::NAIVE)),
                            _ => None,
                        };
                        let _ = tz;
                        match conv {
                            Some(c) => outs.push(Value::lit(c)),
                            None => outs.push(Value::data(t)),
                        }
                        continue;
                    }
                    if let Some(s0) = intern::key_as_str(k) {
                        match chrono::fromisoformat(kind, &s0) {
                            Some(c) => outs.push(Value::lit(c)),
                            None if !s0.trim().starts_with(|c: char| c.is_ascii_digit()) => bad += 1,
                            None => outs.push(Value::data(t)),
                        }
                        continue;
                    }
                    outs.push(Value::data(t));
                }
                if bad > 0 {
                    let loc = self.m.pyd_field;
                    let code = match t {
                        "datetime" => "datetime_from_date_parsing",
                        "date" => "date_from_datetime_parsing",
                        _ => "time_parsing",
                    };
                    self.pyd_err(loc, code, bad == lits.len(), false);
                }
                join_values(outs)
            }
            Ann::Scalar(t) => {
                let t = *t;
                // (the value's literal part: its objects, if any, are checked below)
                if val.kv.is_literal() && !val.kv.is_bot() && matches!(t, "int" | "float" | "Decimal") {
                    let lits = crate::domain::kv_literals(&val.kv);
                    let fails = |k: KeyId| {
                        let c = coerce_kv(t, &KeyVal::Lit(k));
                        let k2 = c.as_lit().unwrap();
                        match t {
                            "int" => intern::key_as_int(k2).is_none(),
                            "Decimal" => crate::models::pynum::as_dec(k2).is_none(),
                            _ => !intern::key_is_number(k2),
                        }
                    };
                    let bad: Vec<KeyId> = lits.iter().filter(|k| fails(*k)).collect();
                    if !bad.is_empty() {
                        let loc = self.m.pyd_field;
                        // (an int from a float with a fractional part: int_from_float)
                        let code_of = |k: KeyId| match t {
                            "int" if intern::with_key(k, |kk| matches!(kk, intern::Key::Float(_))) => "int_from_float",
                            "int" => "int_parsing",
                            "Decimal" => "decimal_parsing",
                            _ => "float_parsing",
                        };
                        let mut codes: Vec<&str> = bad.iter().map(|k| code_of(*k)).collect();
                        codes.sort();
                        codes.dedup();
                        let sure = bad.len() == lits.len() && codes.len() == 1 && val.pts.is_empty();
                        for code in codes {
                            self.pyd_err(loc, code, sure, false);
                        }
                    }
                }
                // an unknown value (a text from a file, any input) may not convert: a parsing error,
                // or a type error when it may be no string either
                if matches!(t, "int" | "float" | "bool") {
                    // (a value already of the type — validated before, `int(x)` — converts)
                    // (`len(x)`, a frame's row count: ints)
                    let int_label = |d: &str| matches!(d, "int" | "bool" | "len" | "rows");
                    let of_type = |d: &str| d == t || (t == "float" && int_label(d)) || (t == "int" && int_label(d)) || d.starts_with('\u{1}');
                    let data_part = !val.kv.is_bot() && !val.kv.is_literal() && !matches!(&val.kv, KeyVal::Data(d) if of_type(&intern::str_get(*d)));
                    let obj_part = val.pts.iter().any(|o| matches!(self.kind(o), Kind::Unknown));
                    if data_part || obj_part {
                        let loc = self.m.pyd_field;
                        let code = match t {
                            "int" => "int_parsing",
                            "float" => "float_parsing",
                            _ => "bool_parsing",
                        };
                        self.pyd_err(loc, code, false, false);
                        if t == "int" {
                            self.pyd_err(loc, "int_from_float", false, false);
                        }
                        let known_str = matches!(&val.kv, KeyVal::Data(d) if &*intern::str_get(*d) == "str") && !obj_part;
                        if data_part && !known_str {
                            let tcode = match t {
                                "int" => "int_type",
                                "float" => "float_type",
                                _ => "bool_type",
                            };
                            self.pyd_err(loc, tcode, false, false);
                        }
                    }
                }
                // an unknown value validated as a Decimal is a Decimal (not the str it came as)
                if t == "Decimal" && !(val.pts.is_empty() && val.kv.is_literal()) {
                    return Value::data("Decimal");
                }
                // objects validated as a scalar: the scalar (a container is an error; something
                // unknown may be one)
                if matches!(t, "str" | "int" | "float" | "bool") && !val.pts.is_empty() {
                    let containers = val.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance)).count();
                    let total = val.pts.len() + usize::from(!val.kv.is_bot());
                    if containers > 0 || val.pts.iter().any(|o| matches!(self.kind(o), Kind::Unknown)) {
                        let loc = self.m.pyd_field;
                        let code = match t {
                            "str" => "string_type",
                            "int" => "int_type",
                            "float" => "float_type",
                            _ => "bool_type",
                        };
                        self.pyd_err(loc, code, containers == total, false);
                    }
                    let rest = Value::kv_only(coerce_kv(t, &val.kv));
                    return if containers == total { rest } else { rest.join(&Value::data(t)) };
                }
                Value::with_kv(val.pts.clone(), coerce_kv(t, &val.kv))
            }
            Ann::Enum(c) => {
                let vk = key_str_lit("value");
                let members: Vec<(ObjId, Option<KeyId>)> = self.enum_members.get(c).cloned().unwrap_or_default().iter().map(|(_, m)| (*m, state.slot(*m, Ns::Attr, vk).kv.as_lit())).collect();
                let all = Value::objs(crate::domain::ObjSet::from_vec(members.iter().map(|(m, _)| *m).collect()));
                let mut out = Value::new();
                let (mut bad, mut total, mut unsure) = (0, 0, false);
                for o in val.pts.iter() {
                    total += 1;
                    if members.iter().any(|(m, _)| *m == o) {
                        out = out.join(&Value::obj(o));
                    } else {
                        // (something else: a member, or an error, as the data has it)
                        unsure = true;
                        out = out.join(&all);
                    }
                }
                if val.kv.is_literal() {
                    for k in crate::domain::kv_literals(&val.kv).iter() {
                        total += 1;
                        match members.iter().find(|(_, mv)| *mv == Some(k)) {
                            Some((m, _)) => out = out.join(&Value::obj(*m)),
                            None => {
                                // a value of another type may be coerced (lax mode): not decided
                                let same_type = members.iter().all(|(_, mv)| mv.map(|x| (intern::key_as_str(x).is_some() && intern::key_as_str(k).is_some()) || (intern::key_as_int(x).is_some() && intern::key_as_int(k).is_some() && !intern::with_key(k, |kk| matches!(kk, intern::Key::Float(_) | intern::Key::Bool(_))))).unwrap_or(false));
                                if same_type {
                                    bad += 1;
                                } else {
                                    unsure = true;
                                    out = out.join(&all);
                                }
                            }
                        }
                    }
                } else if !val.kv.is_bot() {
                    total += 1;
                    unsure = true;
                    out = out.join(&all);
                }
                if bad > 0 || unsure {
                    let loc = self.m.pyd_field;
                    self.pyd_err(loc, "enum", bad == total && bad > 0, false);
                }
                out
            }
            Ann::TypedDict(td) => {
                let td = *td;
                let fields = self.typed_dict_fields(td, state);
                let recs: Vec<ObjId> = val.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Dict | Kind::Unknown)).collect();
                if recs.is_empty() {
                    return val.clone();
                }
                let recv = Value::objs(crate::domain::ObjSet::from_vec(recs.clone()));
                let nid = self.alloc_at(Kind::Dict, node, state, &format!("dict@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("td{}", td)));
                let mut row = Row::new();
                for (name, required, fann) in fields {
                    let k = key_str_lit(&name);
                    let definite = recs.iter().all(|o| state.row(*o, Ns::Item).definite.contains(k));
                    let maybe = recs.iter().any(|o| state.row(*o, Ns::Item).present.contains(k) || state.row(*o, Ns::Item).open());
                    let v = if required && !definite {
                        let saved = self.verdict_guarded;
                        self.verdict_guarded |= self.validation_caught();
                        let v = self.load_item(&recv, Ns::Item, &Value::lit(k), node, state, true);
                        self.verdict_guarded = saved;
                        v
                    } else if maybe {
                        self.pyd_read_guarded(&recs, k, state)
                    } else {
                        continue;
                    };
                    let v = self.pyd_convert(&fann, &v, state, node);
                    row = row.store(k, required || definite);
                    state.set_slot(nid, Ns::Item, k, v, true);
                }
                state.set_row(nid, Ns::Item, row);
                Value::obj(nid)
            }
            Ann::List(inner, kind) if !matches!(**inner, Ann::Other) => {
                // a scalar is no container (str / bytes: an Iterable's items, a Sequence's
                // sequence_str error)
                if !val.kv.is_bot() {
                    let is_text = |k: KeyId| intern::key_is_str(k) || intern::with_key(k, |kk| matches!(kk, intern::Key::Bytes(_)));
                    let code_of = |text: bool| -> Option<&'static str> {
                        Some(match *kind {
                            "tuple" => "tuple_type",
                            "set" => "set_type",
                            "frozenset" => "frozen_set_type",
                            "sequence" if text => "sequence_str",
                            "sequence" => "is_instance_of",
                            "iterable" if text => return None,
                            "iterable" => "iterable_type",
                            _ => "list_type",
                        })
                    };
                    let loc = self.m.pyd_field;
                    if val.kv.is_literal() {
                        let lits = crate::domain::kv_literals(&val.kv);
                        let codes: Vec<Option<&'static str>> = lits.iter().map(|k| code_of(is_text(k))).collect();
                        let mut cs: Vec<&'static str> = codes.iter().flatten().copied().collect();
                        cs.sort();
                        cs.dedup();
                        let sure = codes.iter().all(|c| c.is_some()) && cs.len() == 1 && val.pts.is_empty();
                        for c in cs {
                            self.pyd_err(loc, c, sure, false);
                        }
                    } else {
                        // (an unknown scalar: some error of the container, which one not known)
                        let mut cs: Vec<&'static str> = [code_of(true), code_of(false)].into_iter().flatten().collect();
                        cs.dedup();
                        for c in cs {
                            self.pyd_err(loc, c, false, false);
                        }
                    }
                }
                let mut outs: Vec<Value> = Vec::new();
                for o in val.pts.iter() {
                    if !matches!(self.kind(o), Kind::List | Kind::Tuple | Kind::Set | Kind::Unknown) {
                        outs.push(Value::obj(o));
                        continue;
                    }
                    let it = self.iterate(&Value::obj(o), node, state);
                    // (one list per input list: nested lists in one validation stay apart)
                    let ctx = match self.m.pyd_inst {
                        Some(i) => format!("pyl{}i{}", o, i),
                        None => format!("pyl{}", o),
                    };
                    match it.items {
                        Some(items) => {
                            let conv: Vec<Value> = items.iter().map(|x| self.pyd_convert(inner, x, state, node)).collect();
                            outs.push(self.pyd_list(Some(conv), Value::new(), node, state, &ctx));
                        }
                        None => {
                            let e = match it.elem {
                                crate::interp::funcs::Elem::One(v) => v,
                                _ => Value::data("elem"),
                            };
                            let ce = self.pyd_convert(inner, &e, state, node);
                            outs.push(self.pyd_list(None, ce, node, state, &ctx));
                        }
                    }
                }
                if val.pts.is_empty() {
                    return val.clone();
                }
                join_values(outs)
            }
            Ann::Dict(inner) if !matches!(**inner, Ann::Other) => {
                let mut outs: Vec<Value> = Vec::new();
                for o in val.pts.iter() {
                    if matches!(self.kind(o), Kind::Unknown) {
                        // an open mapping: any key, each value validated
                        let row = state.row(o, Ns::Item);
                        let e = self.path_placeholder_value(o, Ns::Item, intern::STAR, &row, state);
                        let ce = self.pyd_convert(inner, &e, state, node);
                        let nid = self.alloc_at(Kind::Dict, node, state, &format!("dict@L{}", node.line), Payload::None, None, Ns::Item, None, Some(&format!("pyddict{}", o)));
                        state.set_row(nid, Ns::Item, Row::new().data_store(crate::domain::Source::new(&format!("dict@L{}", node.line), crate::domain::SourceKind::Data)));
                        state.set_slot(nid, Ns::Item, intern::STAR, ce, true);
                        outs.push(Value::obj(nid));
                        continue;
                    }
                    if self.kind(o) != Kind::Dict {
                        outs.push(Value::obj(o));
                        continue;
                    }
                    let nid = self.alloc_at(Kind::Dict, node, state, &format!("dict@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("pyddict{}", o)));
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
    /// an include / exclude rowan cannot read: any field may be left out
    pub unknown_sel: bool,
    /// the default dump, under a wrap model serializer
    pub raw: bool,
    /// `mode="json"` / `model_dump_json`: values made JSON-compatible
    pub json: bool,
}

/// The values of a `Literal[...]` annotation.
fn literal_tags(e: &ast::Expr) -> Option<Vec<KeyId>> {
    let ast::Expr::Subscript(sb) = e else { return None };
    if dotted_last(&sb.value) != "Literal" {
        return None;
    }
    let elts: Vec<&ast::Expr> = match &*sb.slice {
        ast::Expr::Tuple(t) => t.elts.iter().collect(),
        other => vec![other],
    };
    elts.into_iter().map(literal_key).collect()
}

fn subst(a: &Ann, bind: &[(String, Ann)]) -> Ann {
    match a {
        Ann::Var(n) => bind.iter().find(|(m, _)| m == n).map(|(_, x)| x.clone()).unwrap_or(Ann::Other),
        Ann::List(x, k) => Ann::List(Box::new(subst(x, bind)), k),
        Ann::Dict(x) => Ann::Dict(Box::new(subst(x, bind))),
        Ann::Optional(x) => Ann::Optional(Box::new(subst(x, bind))),
        Ann::Union(xs, d) => Ann::Union(xs.iter().map(|x| subst(x, bind)).collect(), d.clone()),
        other => other.clone(),
    }
}

/// pydantic's lax coercion of known literals to a scalar annotation (`1` → `1.0` for
/// `float`, `"3"` → `3` for `int`, `"true"` → `True`); a literal that does not convert stays
/// (validation fails there).
fn coerce_kv(t: &str, kv: &KeyVal) -> KeyVal {
    let is_bool = |k: KeyId| intern::with_key(k, |x| matches!(x, intern::Key::Bool(_)));
    let as_float = |k: KeyId| intern::with_key(k, |x| match x {
        intern::Key::Float(b) => Some(f64::from_bits(*b)),
        _ => None,
    });
    let conv = |k: KeyId| -> KeyId {
        let as_str = intern::key_as_str(k);
        let as_int = intern::key_as_int(k);
        match t {
            "float" => {
                if let Some(i) = as_int.filter(|_| !is_bool(k)) {
                    return intern::key_float(i as f64);
                }
                if let Some(x) = as_str.as_ref().and_then(|s0| s0.trim().parse::<f64>().ok()) {
                    return intern::key_float(x);
                }
                k
            }
            "int" => {
                if let Some(x) = as_str.as_ref().and_then(|s0| s0.trim().parse::<i64>().ok()) {
                    return intern::key_int(x);
                }
                if is_bool(k) {
                    return intern::key_int(if k == intern::key_bool(true) { 1 } else { 0 });
                }
                if let Some(f) = as_float(k) {
                    if f.fract() == 0.0 {
                        return intern::key_int(f as i64);
                    }
                }
                k
            }
            "Decimal" => {
                use crate::models::pynum;
                if pynum::as_dec(k).is_some() {
                    return k;
                }
                if let Some(s0) = as_str.as_ref() {
                    return pynum::Dec::parse(s0.trim()).map(pynum::dec_key).unwrap_or(k);
                }
                if let Some(i) = as_int.filter(|_| !is_bool(k)) {
                    return pynum::dec_key(pynum::Dec::from_int(i as i128));
                }
                // a float goes through its repr (`0.1` → `Decimal('0.1')`)
                if let Some(f) = as_float(k) {
                    return pynum::Dec::parse(&crate::models::pyvalues::float_repr(f)).map(pynum::dec_key).unwrap_or(k);
                }
                k
            }
            "bool" => match as_str.as_deref().map(|x| x.to_lowercase()) {
                Some(x) if matches!(x.as_str(), "true" | "1" | "yes" | "on" | "t" | "y") => intern::key_bool(true),
                Some(x) if matches!(x.as_str(), "false" | "0" | "no" | "off" | "f" | "n") => intern::key_bool(false),
                _ => match as_int {
                    Some(1) => intern::key_bool(true),
                    Some(0) => intern::key_bool(false),
                    _ => k,
                },
            },
            _ => k,
        }
    };
    match kv {
        KeyVal::Lit(k) => KeyVal::Lit(conv(*k)),
        KeyVal::Lits(ks) => KeyVal::lits(KeySet::from_iter(ks.iter().map(conv))),
        other => other.clone(),
    }
}

/// A number / string literal (negative numbers too).
fn const_key(e: &ast::Expr) -> Option<KeyId> {
    Some(match e {
        ast::Expr::StringLiteral(s0) => key_str_lit(s0.value.to_str()),
        ast::Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(i) => intern::key_int(i.as_i64()?),
            ast::Number::Float(f) => intern::key_float(*f),
            _ => return None,
        },
        ast::Expr::UnaryOp(u) if matches!(u.op, ast::UnaryOp::USub) => match const_key(&u.operand)? {
            k => match intern::key_get(k) {
                intern::Key::Int(i) => intern::key_int(-i),
                intern::Key::Float(b) => intern::key_float(-f64::from_bits(b)),
                _ => return None,
            },
        },
        _ => return None,
    })
}

/// The constraints of a `Field(…)` call.
/// The error a known literal gets from `Field` / `Query` constraints (`gt`, `min_length`,
/// `pattern`, …), if it breaks one.
pub fn literal_breaks(cons: &[(String, KeyId)], k: KeyId) -> Option<&'static str> {
    let num = |k: KeyId| -> Option<f64> {
        intern::with_key(k, |kk| match kk {
            intern::Key::Int(i) => Some(*i as f64),
            intern::Key::Float(b) => Some(f64::from_bits(*b)),
            _ => None,
        })
    };
    let len = intern::key_as_str(k).map(|s| s.chars().count());
    for (c, lim) in cons {
        let bad = match (c.as_str(), len, num(k), intern::key_as_int(*lim), num(*lim)) {
            ("min_length", Some(l), _, Some(m), _) => (l as i64) < m,
            ("max_length", Some(l), _, Some(m), _) => (l as i64) > m,
            ("gt", _, Some(x), _, Some(m)) => !(x > m),
            ("ge", _, Some(x), _, Some(m)) => !(x >= m),
            ("lt", _, Some(x), _, Some(m)) => !(x < m),
            ("le", _, Some(x), _, Some(m)) => !(x <= m),
            ("pattern", Some(_), _, _, _) => {
                let text = intern::key_as_str(k).unwrap();
                match intern::key_as_str(*lim).and_then(|p| crate::models::pyre::Regex::new(&p, 0)) {
                    Some(re) => {
                        let cs: Vec<char> = text.chars().collect();
                        // (pydantic searches: `re.search`, anchors as the pattern says)
                        matches!(re.search(&cs, 0, cs.len(), None), Ok(None))
                    }
                    None => false,
                }
            }
            _ => false,
        };
        if bad {
            return Some(match (c.as_str(), len.is_some()) {
                ("min_length", true) => "string_too_short",
                ("max_length", true) => "string_too_long",
                ("pattern", _) => "string_pattern_mismatch",
                ("gt", _) => "greater_than",
                ("ge", _) => "greater_than_equal",
                ("lt", _) => "less_than",
                _ => "less_than_equal",
            });
        }
    }
    None
}

pub fn field_constraints(c: &ast::ExprCall) -> Vec<(String, KeyId)> {
    let mut out = Vec::new();
    for k in c.arguments.keywords.iter() {
        let Some(n) = k.arg.as_ref().map(|x| x.to_string()) else { continue };
        if matches!(n.as_str(), "min_length" | "max_length" | "gt" | "ge" | "lt" | "le" | "pattern") {
            if let Some(v) = const_key(&k.value) {
                out.push((n, v));
            }
        }
    }
    out
}

/// `Annotated[str, Field(min_length=3)]`: the Field's constraints.
fn annotated_constraints(ann: &ast::Expr) -> Vec<(String, KeyId)> {
    let ast::Expr::Subscript(sb) = ann else { return Vec::new() };
    if dotted_last(&sb.value) != "Annotated" {
        return Vec::new();
    }
    let ast::Expr::Tuple(t) = &*sb.slice else { return Vec::new() };
    let mut out = Vec::new();
    for e in t.elts.iter().skip(1) {
        if let ast::Expr::Call(c) = e {
            if dotted_last(&c.func) == "Field" {
                out.extend(field_constraints(c));
            }
        }
    }
    out
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

fn config_of(v: &ast::Expr, extra: &mut Extra, populate: &mut bool, frozen: &mut bool) {
    match v {
        ast::Expr::Call(c) => {
            if let Some(e) = kw(c, "extra").and_then(extra_of) {
                *extra = e;
            }
            if let Some(b) = kw(c, "populate_by_name").and_then(bool_lit) {
                *populate = b;
            }
            if let Some(b) = kw(c, "frozen").and_then(bool_lit) {
                *frozen = b;
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
                    "frozen" => {
                        if let Some(b) = bool_lit(&item.value) {
                            *frozen = b;
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// A boolean option of `ConfigDict(…)` / a config dict literal.
fn config_flag(v: &ast::Expr, name: &str) -> bool {
    match v {
        ast::Expr::Call(c) => kw(c, name).and_then(bool_lit).unwrap_or(false),
        ast::Expr::Dict(d) => d.items.iter().any(|it| it.key.as_ref().and_then(str_lit).as_deref() == Some(name) && bool_lit(&it.value) == Some(true)),
        _ => false,
    }
}

/// `SettingsConfigDict(env_prefix=…, env_nested_delimiter=…)` (also a dict literal).
fn settings_of(v: &ast::Expr, settings: &mut Option<(String, Option<String>)>) {
    let Some(st) = settings.as_mut() else { return };
    match v {
        ast::Expr::Call(c) => {
            if let Some(p) = kw(c, "env_prefix").and_then(str_lit) {
                st.0 = p;
            }
            if let Some(d) = kw(c, "env_nested_delimiter").and_then(str_lit) {
                st.1 = Some(d);
            }
        }
        ast::Expr::Dict(d) => {
            for item in d.items.iter() {
                match item.key.as_ref().and_then(str_lit).as_deref() {
                    Some("env_prefix") => {
                        if let Some(p) = str_lit(&item.value) {
                            st.0 = p;
                        }
                    }
                    Some("env_nested_delimiter") => st.1 = str_lit(&item.value),
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
        if self.m.pyd_depth > 6 {
            let did = self.alloc_at(Kind::Dict, node, state, &format!("model_dump@L{}", node.line), Payload::None, None, Ns::Item, None, Some("dumpdeep"));
            state.set_row(did, Ns::Item, Row::new().data_store(crate::domain::Source::new(&format!("model_dump@L{}", node.line), crate::domain::SourceKind::Data)));
            return Value::obj(did);
        }
        self.m.pyd_depth += 1;
        let out = self.pyd_dump_inner(oid, opts, state, node);
        self.m.pyd_depth -= 1;
        out
    }

    fn pyd_dump_inner(&mut self, oid: ObjId, opts: &DumpOpts, state: &mut State, node: &Node) -> Value {
        let cls = match self.kind(oid) {
            Kind::Instance => self.instance_class(oid),
            _ => None,
        };
        // a root model dumps to its root
        if let Some(c) = cls.filter(|c| is_model(self, *c)) {
            let pc = self.pyd_class(c, state);
            if pc.root {
                let ann = pc.fields.iter().find(|f| f.name == "root").map(|f| f.ann.clone()).unwrap_or(Ann::Other);
                let v = self.load_attr(&Value::obj(oid), "root", node, state, false);
                return self.pyd_dump_value(&ann, &v, opts, state, node);
            }
        }
        // a model serializer decides the dump (`mode="wrap"`: around the default one)
        if let Some(c) = cls.filter(|c| is_model(self, *c)) {
            if !opts.raw {
                if let Some(ms) = self.pyd_class(c, state).model_serializer.clone() {
                    let mut args = Vec::new();
                    if ms.mode == "wrap" {
                        let h = self.alloc_raw(Kind::Method, &format!("pyddump:{}", oid), "", true, "serializer handler", Payload::Method("pydantic.dumphandler".into(), Value::obj(oid)));
                        args.push(Value::obj(h));
                    }
                    return self.call_user(ms.func, Some(Value::obj(oid)), args, IndexMap::new(), vec![], vec![], state, node, None, false);
                }
            }
        }
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
            let v = if pc.computed.contains(&name) {
                self.load_attr(&Value::obj(oid), &name, node, state, false)
            } else {
                state.slot(oid, Ns::Attr, nk)
            };
            let mut sub = opts.sub_include.iter().find(|(k, _)| *k == nk).map(|(_, o)| o.clone()).unwrap_or_else(|| DumpOpts { by_alias: opts.by_alias, exclude_none: opts.exclude_none, exclude_unset: opts.exclude_unset, exclude_defaults: opts.exclude_defaults, ..Default::default() });
            sub.json = opts.json;
            let mut dv = self.pyd_dump_value(&ann, &v, &sub, state, node);
            let fser: Vec<Validator> = pc.field_serializers.iter().filter(|s0| s0.fields.contains(&name) || s0.fields.iter().any(|x| x == "*")).cloned().collect();
            for fs in fser {
                let nparams = match &self.func_info(fs.func).node {
                    crate::interp::FuncNode::Def(f) => crate::scopes::all_params(&f.parameters).len(),
                    _ => 2,
                };
                let mut args = vec![dv.clone()];
                if nparams >= 3 {
                    args.push(Value::data("info"));
                }
                dv = self.call_user(fs.func, Some(Value::obj(oid)), args, IndexMap::new(), vec![], vec![], state, node, None, false);
            }
            // per option: Some(true) kept, Some(false) left out, None either
            let none_lit = intern::key_none();
            let only_none = v.pts.is_empty() && v.kv.as_lit() == Some(none_lit);
            let kv_none = v.kv.as_lit() == Some(none_lit) || matches!(&v.kv, KeyVal::Lits(ks) if ks.contains(none_lit));
            let unknown_part = (!v.kv.is_literal() && !v.kv.is_bot()) || v.pts.iter().any(|o| matches!(self.kind(o), Kind::Unknown | Kind::External));
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
                meet(match self.m.pyd_set.get(&oid) {
                    Some((sure, maybe)) => {
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
            if opts.unknown_sel {
                meet(None);
            }
            if keep == Some(false) {
                continue;
            }
            row = row.store(ok, keep == Some(true));
            order.push(ok);
            state.set_slot(did, Ns::Item, ok, dv, true);
        }
        // extra attributes (`extra="allow"`)
        if pc.extra == Extra::Allow {
            let ir = state.row(oid, Ns::Attr);
            let fieldset: KeySet = KeySet::from_iter(pc.fields.iter().map(|f| key_str_lit(&f.name)));
            // (in the instance's insertion order when known: the fields, then the extras)
            let ordered = ir.ordered_keys();
            let keys: Vec<KeyId> = ordered.clone().unwrap_or_else(|| ir.present.iter().collect());
            for k in keys {
                if fieldset.contains(k) || opts.exclude.as_ref().map(|e| e.contains(k)).unwrap_or(false) || opts.include.as_ref().map(|i| !i.contains(k)).unwrap_or(false) {
                    continue;
                }
                row = row.store(k, ir.definite.contains(k));
                if ordered.is_some() {
                    order.push(k);
                }
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
            // JSON mode: a Decimal / Fraction is its text
            let kv = if opts.json && v.kv.is_literal() && crate::domain::kv_literals(&v.kv).iter().any(crate::models::pynum::is_num_key) {
                KeyVal::lits(KeySet::from_iter(crate::domain::kv_literals(&v.kv).iter().map(|k| match crate::models::pynum::str_of(k) {
                    Some(t) => key_str_lit(&t),
                    None => k,
                })))
            } else if opts.json && v.kv.is_literal() && crate::domain::kv_literals(&v.kv).iter().any(crate::models::chrono::is_chrono_key) {
                // JSON mode: ISO text (UTC as `Z`)
                KeyVal::lits(KeySet::from_iter(crate::domain::kv_literals(&v.kv).iter().map(|k| match crate::models::chrono::as_chrono(k) {
                    Some((kind, _, tz)) if kind != crate::models::chrono::DELTA && kind != crate::models::chrono::TZ => {
                        let t = crate::models::chrono::isoformat(k, "T", "auto").unwrap_or_default();
                        let t = if tz == 0 { format!("{}Z", t.trim_end_matches("+00:00")) } else { t };
                        key_str_lit(&t)
                    }
                    _ => k,
                })))
            } else if opts.json && matches!(ann, Ann::Scalar("datetime" | "date" | "time")) && !v.kv.is_literal() {
                crate::domain::DATA
            } else if opts.json && matches!(ann, Ann::Scalar("Decimal")) && !v.kv.is_literal() {
                crate::domain::DATA
            } else {
                v.kv.clone()
            };
            outs.push(Value::kv_only(kv));
        }
        for o in v.pts.iter() {
            // JSON mode: an Enum member is its value
            if opts.json && self.member_key.contains_key(&o) {
                outs.push(state.slot(o, Ns::Attr, key_str_lit("value")));
                continue;
            }
            if is_instance(self, o) {
                outs.push(self.pyd_dump(o, opts, state, node));
                continue;
            }
            // a dataclass (pydantic's or the standard one) dumps as the dict of its fields
            let dc_cls = match self.kind(o) {
                Kind::Instance => self.instance_class(o),
                _ => None,
            };
            if dc_cls.map(|c| !self.dataclass_fields_mro(c).is_empty()).unwrap_or(false) {
                outs.push(crate::models::asdict_of(self, state, node, &Value::obj(o)));
                continue;
            }
            match (self.kind(o), ann) {
                (Kind::List | Kind::Tuple | Kind::Set, Ann::List(inner, _) | Ann::Optional(inner)) => {
                    let inner = match &**inner {
                        Ann::List(i, _) => (**i).clone(),
                        other => other.clone(),
                    };
                    let it = self.iterate(&Value::obj(o), node, state);
                    let ctx = format!("dumpl{}", o);
                    match it.items {
                        Some(items) => {
                            let conv: Vec<Value> = items.iter().map(|x| self.pyd_dump_value(&inner, x, opts, state, node)).collect();
                            outs.push(self.pyd_list(Some(conv), Value::new(), node, state, &ctx));
                        }
                        None => {
                            let e = match it.elem {
                                crate::interp::funcs::Elem::One(v) => v,
                                _ => Value::data("elem"),
                            };
                            let ce = self.pyd_dump_value(&inner, &e, opts, state, node);
                            outs.push(self.pyd_list(None, ce, node, state, &ctx));
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
pub fn key_spec(an: &mut Analyzer, st: &mut State, v: &Value, node: &Node) -> Option<(KeySet, Vec<(KeyId, Value)>)> {
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

/// The keys a nested include / exclude spec names: for a list field `{"__all__": {...}}` is the
/// spec of every item (per-index specs are not read: None).
fn item_spec(an: &mut Analyzer, st: &mut State, sub: &Value, node: &Node) -> Option<KeySet> {
    let (sk, nested) = key_spec(an, st, sub, node)?;
    let all = key_str_lit("__all__");
    if sk.contains(all) {
        if sk.len() != 1 {
            return None;
        }
        let inner = nested.iter().find(|(k, _)| *k == all).map(|(_, v)| v.clone())?;
        return key_spec(an, st, &inner, node).map(|(ks, _)| ks);
    }
    if sk.iter().any(|k| intern::key_as_int(k).is_some()) {
        return None;
    }
    Some(sk)
}

fn dump_opts(an: &mut Analyzer, st: &mut State, c: &Call) -> DumpOpts {
    let flag = |c: &Call, n: &str| c.kwargs.get(&s(n)).map(|v| v.kv.as_lit() == Some(intern::key_bool(true))).unwrap_or(false);
    let mut o = DumpOpts { exclude_none: flag(c, "exclude_none"), exclude_unset: flag(c, "exclude_unset"), exclude_defaults: flag(c, "exclude_defaults"), by_alias: flag(c, "by_alias"), ..Default::default() };
    o.json = c.qual.ends_with("model_dump_json") || c.qual.ends_with(".json") || c.kwargs.get(&s("mode")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_str).as_deref() == Some("json");
    if let Some(v) = c.kwargs.get(&s("include")).cloned() {
        match key_spec(an, st, &v, &c.node) {
            Some((ks, nested)) => {
                o.include = Some(ks);
                for (k, sub) in nested {
                    match item_spec(an, st, &sub, &c.node) {
                        Some(sk) => o.sub_include.push((k, DumpOpts { include: Some(sk), by_alias: o.by_alias, ..Default::default() })),
                        None => o.unknown_sel = true,
                    }
                }
            }
            // an include set rowan cannot read: any field may be left out
            None => o.unknown_sel = true,
        }
    }
    if let Some(v) = c.kwargs.get(&s("exclude")).cloned() {
        match key_spec(an, st, &v, &c.node) {
            Some((ks, nested)) => {
                // `{"addr": {"zip"}}` excludes inside the field, not the field
                let whole: Vec<KeyId> = ks.iter().filter(|k| !nested.iter().any(|(n, sub)| n == k && !sub.pts.is_empty() && sub.kv.is_bot())).collect();
                o.exclude = Some(KeySet::from_iter(whole));
                for (k, sub) in nested {
                    match item_spec(an, st, &sub, &c.node) {
                        Some(sk) => o.sub_include.push((k, DumpOpts { exclude: Some(sk), by_alias: o.by_alias, ..Default::default() })),
                        None => o.unknown_sel = true,
                    }
                }
            }
            None => o.unknown_sel = true,
        }
    }
    // a parameter's selection: decided where the caller passes a literal one
    o
}

/// The model classes a receiver value stands for (a class, an instance, a typed placeholder).
fn recv_classes(an: &Analyzer, v: &Value) -> Vec<ObjId> {
    let mut out = Vec::new();
    for o in v.pts.iter() {
        let c = match an.kind(o) {
            Kind::Class => Some(o),
            Kind::Instance => an.instance_class(o),
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
            // the text of the dump (with its options): `json.loads` gives a copy of it
            let opts = dump_opts(an, st, c);
            let dumps = join_values(recv.pts.iter().map(|o| an.pyd_dump(o, &opts, st, &c.node)).collect::<Vec<_>>());
            // the exact text, when every value is known (`indent=` aside)
            if c.kwargs.get(&s("indent")).is_none() {
                if let Some(t) = crate::models::compact_json_text(an, st, &dumps) {
                    return Value::lit(key_str_lit(&t));
                }
            }
            let ids: Vec<String> = dumps.pts.iter().map(|o| o.to_string()).collect();
            Value::data(&format!("pydjson:{}", ids.join(",")))
        }
        "model_copy" | "copy" => {
            let mut outs = Vec::new();
            for o in recv.pts.iter() {
                let cp = an.copy_object(o, &c.node, st, matches!(c.kwargs.get(&s("deep")).and_then(|v| v.kv.as_lit()), Some(k) if k == intern::key_bool(true)), &mut IndexMap::new());
                if let Some(u) = c.kwargs.get(&s("update")).cloned() {
                    // `update=` sets attributes without validation (on the fresh copy: strongly
                    // for a key the update surely has)
                    let closed_dicts = !u.pts.is_empty() && u.pts.iter().all(|x| an.kind(x) == Kind::Dict && !st.row(x, Ns::Item).open());
                    if closed_dicts {
                        let one = u.pts.len() == 1;
                        for x in u.pts.iter() {
                            let ur = st.row(x, Ns::Item);
                            for k in ur.present.iter() {
                                let sure = one && ur.definite.contains(k);
                                let v = st.slot(x, Ns::Item, k);
                                let r = st.row(cp, Ns::Attr).store(k, sure || st.row(cp, Ns::Attr).definite.contains(k));
                                st.set_row(cp, Ns::Attr, r);
                                st.set_slot(cp, Ns::Attr, k, v, sure);
                            }
                        }
                    } else {
                        an.copy_into(cp, Ns::Attr, &u, st, true, &KeySet::new());
                    }
                }
                // the copy's fields set: the original's, plus what `update=` sets
                if let Some((mut sure, mut maybe)) = an.m.pyd_set.get(&o).cloned() {
                    if let Some(u) = c.kwargs.get(&s("update")) {
                        let one = u.pts.len() == 1;
                        for x in u.pts.iter().filter(|x| an.kind(*x) == Kind::Dict) {
                            let ur = st.row(x, Ns::Item);
                            for k in ur.present.iter() {
                                if one && ur.definite.contains(k) {
                                    sure = sure.insert(k);
                                }
                                maybe = maybe.insert(k);
                            }
                        }
                    }
                    an.m.pyd_set.insert(cp, (sure, maybe));
                }
                outs.push(Value::obj(cp));
            }
            join_values(outs)
        }
        "model_fields_set" | "__fields_set__" => {
            let mut outs = Vec::new();
            for o in recv.pts.iter() {
                // made from a parameter's input: decided where the caller passes a literal one
                match an.m.pyd_set.get(&o).cloned() {
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
            // the attributes that are not fields (`extra="allow"`), else None
            let mut outs = Vec::new();
            for o in recv.pts.iter() {
                let cls = match an.kind(o) {
                    Kind::Instance => an.instance_class(o),
                    _ => None,
                };
                let Some(cls) = cls.filter(|c| is_model(an, *c)) else {
                    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("model_extra@L{}", c.line));
                    let r = Row::new().data_store(crate::domain::Source::new(&format!("model_extra@L{}", c.line), crate::domain::SourceKind::Data));
                    st.set_row(oid, Ns::Item, r);
                    outs.push(Value::obj(oid));
                    continue;
                };
                let pc = an.pyd_class(cls, st);
                if pc.extra != Extra::Allow {
                    outs.push(an.none_v.clone());
                    continue;
                }
                let fieldset: KeySet = KeySet::from_iter(pc.fields.iter().map(|f| key_str_lit(&f.name)));
                let ir = st.row(o, Ns::Attr);
                let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("model_extra@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("extra{}", o)));
                let mut row = Row::new();
                for k in ir.present.iter() {
                    if fieldset.contains(k) {
                        continue;
                    }
                    row = row.store(k, ir.definite.contains(k));
                    st.set_slot(oid, Ns::Item, k, st.slot(o, Ns::Attr, k), true);
                }
                if ir.open() {
                    row = row.copy_from(&ir, false, &fieldset, intern::EMPTY_LENS);
                }
                for k in fieldset.iter() {
                    row = row.delete(k, true);
                }
                st.set_row(oid, Ns::Item, row);
                outs.push(Value::obj(oid));
            }
            join_values(outs)
        }
        "model_post_init" => an.none_v.clone(),
        // a wrap serializer's handler: the default dump
        "dumphandler" => {
            let opts = DumpOpts { raw: true, ..Default::default() };
            join_values(recv.pts.iter().map(|o| an.pyd_dump(o, &opts, st, &c.node)).collect::<Vec<_>>())
        }
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
                let recs: Vec<ObjId> = c.star_kwargs.iter().flat_map(|v| v.pts.iter().collect::<Vec<_>>()).collect();
                for f in pc.fields.clone() {
                    let nk = key_str_lit(&f.name);
                    if let Some(v) = c.kwargs.get(&s(&f.name)) {
                        row = row.store(nk, true);
                        st.set_slot(inst, Ns::Attr, nk, v.clone(), true);
                        continue;
                    }
                    // from `**d`: the key where it is there (by name), else the default
                    let definite = !recs.is_empty() && recs.iter().all(|o| st.row(*o, Ns::Item).definite.contains(nk));
                    let maybe = recs.iter().any(|o| st.row(*o, Ns::Item).present.contains(nk) || st.row(*o, Ns::Item).open());
                    let mut v = Value::new();
                    if maybe {
                        v = an.pyd_read_guarded(&recs, nk, st);
                    }
                    if !definite && !f.required {
                        v = v.join(&an.pyd_default(&f, &c.node, st));
                    }
                    if definite || !f.required {
                        row = row.store(nk, true);
                    } else if maybe {
                        row = row.store(nk, false);
                    } else {
                        continue;
                    }
                    st.set_slot(inst, Ns::Attr, nk, v, true);
                }
                if pc.extra == Extra::Allow {
                    let fieldset: KeySet = KeySet::from_iter(pc.fields.iter().map(|f| key_str_lit(&f.name)));
                    for sk in c.star_kwargs.clone() {
                        st.set_row(inst, Ns::Attr, row.clone());
                        an.copy_into(inst, Ns::Attr, &sk, st, true, &fieldset);
                        row = st.row(inst, Ns::Attr);
                    }
                }
                st.set_row(inst, Ns::Attr, row);
                Value::obj(inst)
            }
            "model_fields" | "__fields__" | "model_computed_fields" => {
                let pc = an.pyd_class(cls, st);
                let names: Vec<String> = if name == "model_computed_fields" { pc.computed.clone() } else { pc.fields.iter().map(|f| f.name.clone()).collect() };
                let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("{}@L{}", name, c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("mf{}", cls)));
                let ks: Vec<KeyId> = names.iter().map(|n| key_str_lit(n)).collect();
                let mut row = Row::new();
                for (k, n) in ks.iter().zip(names.iter()) {
                    row = row.store(*k, true);
                    let fi = an.alloc_raw(Kind::Namespace, &format!("fieldinfo:{}:{}", cls, n), "", true, "FieldInfo", Payload::Method(format!("fieldinfo.{}.{}", cls, n), Value::new()));
                    st.set_row(fi, Ns::Attr, Row::new());
                    st.set_slot(oid, Ns::Item, *k, Value::obj(fi), true);
                }
                st.set_row(oid, Ns::Item, row.with_order(ks));
                Value::obj(oid)
            }
            "model_json_schema" | "schema" => {
                let pc = an.pyd_class(cls, st);
                let by_alias = !matches!(c.kwargs.get(&s("by_alias")).and_then(|v| v.kv.as_lit()), Some(k) if k == intern::key_bool(false));
                let serialization = c.kwargs.get(&s("mode")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_str).map(|m| &*m == "serialization").unwrap_or(false);
                let sid = an.alloc_at(Kind::Dict, &c.node, st, &format!("schema@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("js{}", cls)));
                let pid = an.alloc_at(Kind::Dict, &c.node, st, &format!("properties@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("jsp{}", cls)));
                let mut prow = Row::new();
                let mut req: Vec<Value> = Vec::new();
                let mut props: Vec<(String, bool)> = Vec::new();
                for f in pc.fields.iter() {
                    let alias = if serialization { f.ser_alias.clone() } else { f.alias.clone() };
                    let key = if by_alias { alias.unwrap_or_else(|| f.name.clone()) } else { f.name.clone() };
                    // serialization: a field with a default is always there (required)
                    props.push((key, f.required || serialization));
                }
                if serialization {
                    for cf in &pc.computed {
                        props.push((cf.clone(), true));
                    }
                }
                for (key, required) in &props {
                    let k = key_str_lit(key);
                    prow = prow.store(k, true);
                    let fd = an.alloc_at(Kind::Dict, &c.node, st, &format!("property@L{}", c.line), Payload::None, None, Ns::Item, None, Some(&format!("jsf{}:{}", cls, key)));
                    st.set_row(fd, Ns::Item, Row::new().data_store(crate::domain::Source::new(&format!("schema@L{}", c.line), crate::domain::SourceKind::Data)));
                    st.set_slot(pid, Ns::Item, k, Value::obj(fd), true);
                    if *required {
                        req.push(Value::lit(k));
                    }
                }
                st.set_row(pid, Ns::Item, prow);
                let has_req = !req.is_empty();
                let rl = an.alloc_positional(Kind::List, &c.node, Some(req), Value::new(), crate::domain::DATA, st);
                // the models it refers to (transitively): `$defs`, keyed by class name
                let mut defs: Vec<ObjId> = Vec::new();
                let mut work: Vec<ObjId> = vec![cls];
                while let Some(m) = work.pop() {
                    let mpc = an.pyd_class(m, st);
                    for f in mpc.fields.iter() {
                        let mut a = &f.ann;
                        loop {
                            match a {
                                Ann::List(x, _) | Ann::Dict(x) | Ann::Optional(x) => a = x,
                                _ => break,
                            }
                        }
                        if let Ann::Model(n) = a {
                            if *n != cls && !defs.contains(n) {
                                defs.push(*n);
                                work.push(*n);
                            }
                        }
                    }
                }
                let mut srow = Row::new();
                let mut entries: Vec<(&str, Value)> = vec![("properties", Value::obj(pid)), ("title", Value::lit_str(&an.class_info(cls).name.clone())), ("type", Value::lit_str("object"))];
                if has_req {
                    entries.push(("required", rl));
                }
                if !defs.is_empty() {
                    let did = an.alloc_at(Kind::Dict, &c.node, st, &format!("$defs@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("jsd{}", cls)));
                    let mut drow = Row::new();
                    for d in &defs {
                        let k = key_str_lit(&an.class_info(*d).name.clone());
                        drow = drow.store(k, true);
                        st.set_slot(did, Ns::Item, k, Value::data("schema"), true);
                    }
                    st.set_row(did, Ns::Item, drow);
                    entries.push(("$defs", Value::obj(did)));
                }
                let doc = an.class_info(cls).node.body.first().map(|st0| matches!(st0, ast::Stmt::Expr(e) if matches!(&*e.value, ast::Expr::StringLiteral(_)))).unwrap_or(false);
                if doc {
                    entries.push(("description", Value::data("docstring")));
                }
                for (k, v) in entries {
                    let kk = key_str_lit(k);
                    srow = srow.store(kk, true);
                    st.set_slot(sid, Ns::Item, kk, v, true);
                }
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

/// An attribute of a `FieldInfo` (`fieldinfo.<cls>.<field>`): `default`, `alias`,
/// `is_required()`.
pub fn fieldinfo_attr(an: &mut Analyzer, st: &mut State, q: &str, name: &str, node: &Node) -> Value {
    let mut it = q.splitn(3, '.');
    let (_, cls, field) = (it.next(), it.next().and_then(|x| x.parse::<ObjId>().ok()), it.next().unwrap_or(""));
    let Some(cls) = cls else { return Value::data("FieldInfo") };
    let pc = an.pyd_class(cls, st);
    let Some(f) = pc.fields.iter().find(|f| f.name == field).cloned() else { return Value::data("FieldInfo") };
    match name {
        "default" => match (f.required, f.factory) {
            (true, _) => Value::data("PydanticUndefined"),
            (false, Some(_)) => Value::data("PydanticUndefined"),
            (false, None) => an.pyd_default(&f, node, st),
        },
        "alias" | "validation_alias" | "serialization_alias" => {
            let a = match name {
                "serialization_alias" => f.ser_alias.clone(),
                _ => f.alias.clone(),
            };
            match a {
                Some(x) if f.alts.is_empty() || name != "validation_alias" => Value::lit_str(&x),
                Some(_) => Value::data("AliasChoices"),
                None => an.none_v.clone(),
            }
        }
        "is_required" => an.method_value(&format!("fieldinfo.is_required.{}", f.required), &Value::new()),
        _ => Value::data(&format!("FieldInfo.{}", name)),
    }
}

/// The records of ValidationError number `idx` (`errors()`): one dict per problem, its `loc`
/// a tuple (prefixed by `prefix`: FastAPI's `"body"`), `url` when asked for.
pub fn verr_errors(an: &mut Analyzer, st: &mut State, node: &crate::frontend::Node, idx: usize, url: bool, prefix: Option<KeyId>) -> Value {
    let Some(errs) = an.m.pyd_verrs.get(idx).cloned() else { return Value::data("errors") };
    let line = node.line;
    let c_node = *node;
            let mut items: Vec<Value> = Vec::new();
            for (i, e) in errs.iter().enumerate() {
                let d = an.alloc_at(Kind::Dict, &c_node, st, &format!("error@L{}", line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("verr{}:{}:{}", idx, i, url)));
                let mut row = Row::new();
                // one tuple per record (records made at one site must not share it)
                let lt = an.alloc_at(Kind::Tuple, &c_node, st, &format!("loc@L{}", line), Payload::None, None, Ns::Item, None, Some(&format!("verrloc{}:{}", idx, i)));
                // (objects shared with other uses — past the clone budget — take these facts
                // weakly)
                let dstrong = an.obj(d).single;
                let lstrong = an.obj(lt).single;
                // (the parts of the location, after `prefix`; unknown: no positions)
                let parts: Option<Vec<KeyId>> = match e.loc {
                    Some(k) if intern::with_key(k, |kk| matches!(kk, intern::Key::Tuple(_))) => Some(intern::with_key(k, |kk| if let intern::Key::Tuple(t) = kk { t.clone() } else { vec![] })),
                    Some(k) => Some(vec![k]),
                    None => None,
                };
                match parts {
                    Some(ps) => {
                        let ps: Vec<KeyId> = prefix.into_iter().chain(ps).collect();
                        for (j, p) in ps.iter().enumerate() {
                            st.set_slot(lt, Ns::Pos, intern::key_int(j as i64), Value::lit(*p), lstrong);
                        }
                        st.set_slot(lt, Ns::Elem, intern::STAR, join_values(ps.iter().map(|p| Value::lit(*p)).collect::<Vec<_>>()), lstrong);
                        st.elems.insert(lt, KeyVal::lits(intern::KeySet::from_iter(ps.iter().copied())));
                        an.set_llen(lt, st, Some(ps.len()));
                    }
                    None => {
                        if let Some(p) = prefix {
                            st.set_slot(lt, Ns::Pos, intern::key_int(0), Value::lit(p), lstrong);
                            an.set_prefix(lt, st, 1);
                        } else {
                            an.set_llen(lt, st, None);
                        }
                        st.set_slot(lt, Ns::Elem, intern::STAR, Value::data("loc"), lstrong);
                        st.elems.insert(lt, crate::domain::DATA);
                    }
                }
                let loc = Value::obj(lt);
                let mut entries: Vec<(&str, Value)> = vec![("type", Value::lit_str(e.typ)), ("loc", loc), ("msg", Value::data("msg")), ("input", Value::data("input"))];
                if url {
                    entries.push(("url", Value::data("url")));
                }
                if e.ctx {
                    entries.push(("ctx", Value::data("ctx")));
                }
                for (k, v) in entries {
                    let kk = key_str_lit(k);
                    row = row.store(kk, true);
                    st.set_slot(d, Ns::Item, kk, v, dstrong);
                }
                let row = if dstrong { row } else { st.row(d, Ns::Item).join(&row) };
                st.set_row(d, Ns::Item, row);
                items.push(Value::obj(d));
            }
            if errs.iter().all(|e| e.sure) {
                an.alloc_positional(Kind::List, &c_node, Some(items), Value::new(), crate::domain::DATA, st)
            } else {
                let elem = join_values(items);
                an.alloc_positional(Kind::List, &c_node, None, elem, crate::domain::DATA, st)
            }
}

/// A ValidationError's methods (`pydverr.<idx>.<name>`): `errors()` (one record per
/// problem, in field order; `include_url=False` leaves out `url`), `error_count()`.
pub fn m_pydverr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut it = c.qual.splitn(3, '.');
    let (_, idx, name) = (it.next(), it.next().and_then(|x| x.parse::<usize>().ok()), it.next().unwrap_or(""));
    let Some(errs) = idx.and_then(|i| an.m.pyd_verrs.get(i).cloned()) else { return Value::data("ValidationError") };
    match name {
        "errors" => {
            let url = !matches!(c.kwargs.get(&s("include_url")).and_then(|v| v.kv.as_lit()), Some(k) if k == intern::key_bool(false));
            verr_errors(an, st, &c.node, idx.unwrap(), url, None)
        }
        "error_count" if errs.iter().all(|e| e.sure) => Value::lit(intern::key_int(errs.len() as i64)),
        "title" => Value::data("title"),
        _ => Value::data("ValidationError"),
    }
}

/// `field_info.is_required()`.
pub fn m_fieldinfo(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    match c.qual.as_str() {
        "fieldinfo.is_required.true" => Value::lit(intern::key_bool(true)),
        "fieldinfo.is_required.false" => Value::lit(intern::key_bool(false)),
        _ => Value::data("FieldInfo"),
    }
}

/// `TypeAdapter(T)`: remembers T; `validate_python(data)` / `validate_json(text)` validate.
pub fn m_type_adapter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(ast_call) = c.call_ast else { return Value::data("TypeAdapter") };
    let Some(t) = ast_call.arguments.args.first() else { return Value::data("TypeAdapter") };
    let module = an.frame().module;
    let ann = an.pyd_ann(t, module, st);
    let idx = an.m.pyd_adapters.len();
    an.m.pyd_adapters.push(ann);
    Value::obj(an.alloc_raw(Kind::Namespace, &format!("typeadapter:{}", idx), "", true, "TypeAdapter", Payload::Method(format!("typeadapter.{}", idx), Value::new())))
}

/// `adapter.validate_python(data)` and friends (`typeadapter.<idx>.<method>`).
pub fn adapter_call(an: &mut Analyzer, st: &mut State, idx: usize, method: &str, c: &Call) -> Value {
    let ann = an.m.pyd_adapters.get(idx).cloned().unwrap_or(Ann::Other);
    match method {
        "validate_python" | "validate_strings" => {
            let data = c.arg(0, None).unwrap_or_else(|| Value::data("data"));
            // a known literal the scalar type cannot take: a ValidationError
            if let (Ann::Scalar(t), true) = (&ann, data.pts.is_empty()) {
                let lits = crate::domain::kv_literals(&data.kv);
                if !lits.is_empty() && data.kv.is_literal() {
                    let fails = |k: KeyId| {
                        let conv = coerce_kv(t, &KeyVal::Lit(k));
                        match *t {
                            "int" => intern::key_as_int(conv.as_lit().unwrap()).is_none(),
                            "float" => !intern::key_is_number(conv.as_lit().unwrap()),
                            _ => false,
                        }
                    };
                    let bad: Vec<KeyId> = lits.iter().filter(|k| fails(*k)).collect();
                    if bad.len() == lits.len() {
                        an.raise_here("ValidationError", st);
                        return Value::new();
                    }
                    if !bad.is_empty() {
                        an.may_raise_here("ValidationError", st);
                        let good: KeySet = KeySet::from_iter(lits.iter().filter(|k| !bad.contains(k)));
                        return an.pyd_convert(&ann, &Value::kv_only(KeyVal::lits(good)), st, &c.node);
                    }
                }
            }
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

/// `json.loads(m.model_dump_json())`: a copy of the dump.
pub fn json_of_models(an: &mut Analyzer, st: &mut State, c: &Call, ms: &[ObjId]) -> Value {
    join_values(ms.iter().map(|o| Value::obj(an.copy_object(*o, &c.node, st, true, &mut IndexMap::new()))).collect::<Vec<_>>())
}

pub fn install(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["pydantic.*"], m_instance);
    r(&["fieldinfo.*"], m_fieldinfo);
    r(&["pydverr.*"], m_pydverr);
    r(&["pydantic.cls.*"], m_class);
    r(&["pydantic.TypeAdapter", "pydantic.type_adapter.TypeAdapter"], m_type_adapter);
    r(&["pydantic.Field", "pydantic.fields.Field", "pydantic.PrivateAttr", "pydantic.ConfigDict", "pydantic_settings.SettingsConfigDict"], crate::models::noeffect);
}

/// An assignment to an attribute of a pydantic model instance: a frozen model raises a
/// ValidationError, a field assigned joins `model_fields_set` (and is validated with
/// `validate_assignment`), a private attribute is kept apart. True: handled.
pub fn model_setattr(an: &mut Analyzer, oid: ObjId, name: &str, v: &Value, strong: bool, node: &Node, state: &mut State) -> bool {
    let nk = key_str_lit(name);
    // a frozen pydantic model: assignment raises a ValidationError; a field assigned is
    // in `model_fields_set` from then on
    if !name.starts_with('_') {
        if let Some(cls) = an.instance_class(oid) {
            if crate::models::pydantic::is_model(an, cls) {
                let pc = an.pyd_class(cls, state);
                if pc.frozen {
                    an.raise_here("ValidationError", state);
                    return true;
                }
                if pc.fields.iter().any(|f| f.name == name) {
                    if let Some((sure, maybe)) = an.m.pyd_set.get(&oid).cloned() {
                        let ns = if strong { sure.insert(nk) } else { sure };
                        an.m.pyd_set.insert(oid, (ns, maybe.insert(nk)));
                    }
                }
                match an.pyd_assign(oid, cls, name, v, node, state) {
                    None => return true,
                    Some(nv) if pc.validate_assignment => {
                        let r = state.row(oid, Ns::Attr).store(nk, strong);
                        state.set_row(oid, Ns::Attr, r);
                        state.set_slot(oid, Ns::Attr, nk, nv, strong);
                        return true;
                    }
                    Some(_) => {}
                }
            }
        }
    }
    if name.starts_with('_') {
        let cls = an.instance_class(oid);
        if let Some(cls) = cls {
            if an.pyd_is_private(cls, name, state) {
                state.set_slot(oid, Ns::Attr, crate::models::pydantic::priv_key(name), v.clone(), strong);
                return true;
            }
        }
    }
    false
}
