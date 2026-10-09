//! FastAPI (SPECIFICATION §11.2): apps and routers record their routes; a handler is an entry
//! point whose parameters are bound the way FastAPI binds them (a pydantic model: validated
//! from an open request body; `dict` / `Body()`: the open body; `Depends(g)`: g's result, g's
//! own parameters bound the same way; `Query` / `Path` / `Header` and plain scalars: open
//! values joined with their defaults; `Request` / `BackgroundTasks`: small objects). A
//! `TestClient` call dispatches to the matching route with the request's values: the handler
//! runs on them, an `HTTPException` becomes the response, `.json()` is the returned value as
//! FastAPI serialises it (through `response_model` / the return annotation).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, KeyVal, Ns, ObjId, Row, Source, SourceKind, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, StrId};
use crate::interp::{dotted_last, Analyzer, Call, Frame, FrameKind, FuncNode, Payload};
use crate::models::pydantic::{Ann, DumpOpts};
use crate::state::{Kind, State};

#[derive(Clone, Debug)]
pub struct FaRoute {
    pub app: ObjId,
    pub methods: Vec<String>,
    pub path: Option<String>,
    pub func: Option<ObjId>,
    /// the decorator call (response_model, response_model_exclude_none, …) and its module
    pub call: Option<&'static ast::ExprCall>,
    pub module: ObjId,
}

/// A concrete request (a `TestClient` call): the JSON body, the query and header mappings
/// and the path parameters taken from the URL.
#[derive(Clone, Default)]
pub struct Req {
    pub json: Option<Value>,
    pub query: Option<Value>,
    pub url_query: Vec<(String, String)>,
    pub headers: Option<Value>,
    pub path: Vec<(String, String)>,
}

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

fn fa_obj(an: &mut Analyzer, st: &mut State, node: &Node, qual: &str, v: Value, ctx: &str) -> Value {
    let oid = an.alloc_at(Kind::Namespace, node, st, qual, Payload::Method(qual.to_string(), v.clone()), Some(Row::new()), Ns::Attr, None, Some(ctx));
    an.obj_mut(oid).payload = Payload::Method(qual.to_string(), v);
    Value::obj(oid)
}

fn payload_of(an: &Analyzer, oid: ObjId) -> Option<(String, Value)> {
    match &an.obj(oid).payload {
        Payload::Method(q, v) if q.starts_with("fastapi.") && an.kind(oid) == Kind::Namespace => Some((q.clone(), v.clone())),
        _ => None,
    }
}

fn kw<'a>(c: &'a ast::ExprCall, name: &str) -> Option<&'a ast::Expr> {
    c.arguments.keywords.iter().find(|k| k.arg.as_ref().map(|a| a.as_str() == name).unwrap_or(false)).map(|k| &k.value)
}

fn str_of(v: &Value) -> Option<String> {
    if !v.pts.is_empty() {
        return None;
    }
    v.kv.as_lit().and_then(intern::key_as_str).map(|x| x.to_string())
}

/// `/items/{item_id}` against `/items/3`: the path parameters, or None.
fn match_path(tmpl: &str, url: &str) -> Option<Vec<(String, String)>> {
    let t: Vec<&str> = tmpl.trim_end_matches('/').split('/').collect();
    let u: Vec<&str> = url.trim_end_matches('/').split('/').collect();
    let mut out = Vec::new();
    for (i, seg) in t.iter().enumerate() {
        if let Some(name) = seg.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            // `{file_path:path}` takes the rest
            if let Some((n, conv)) = name.split_once(':') {
                if conv == "path" {
                    out.push((n.to_string(), u.get(i..).map(|r| r.join("/")).unwrap_or_default()));
                    return Some(out);
                }
                out.push((n.to_string(), u.get(i)?.to_string()));
                continue;
            }
            out.push((name.to_string(), u.get(i)?.to_string()));
        } else if u.get(i) != Some(seg) {
            return None;
        }
    }
    if t.len() != u.len() {
        return None;
    }
    Some(out)
}

impl Analyzer {
    fn eval_in_module(&mut self, module: ObjId, e: &'static ast::Expr, state: &mut State) -> Value {
        let mi = self.module_info(module);
        self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), module, "fastapi", true, "<fastapi>"));
        let v = self.eval(e, state);
        self.frames.pop();
        v
    }

    /// All routes of `app`: its own and those of the routers included in it (with prefixes),
    /// as (full path, route index).
    pub fn fa_routes_of(&self, app: ObjId) -> Vec<(Option<String>, usize)> {
        let mut out = Vec::new();
        let mut work: Vec<(ObjId, String, u32)> = vec![(app, String::new(), 0)];
        while let Some((a, prefix, depth)) = work.pop() {
            if depth > 8 {
                continue;
            }
            let own = format!("{}{}", prefix, self.fa_prefix.get(&a).cloned().unwrap_or_default());
            for (i, r) in self.fa_routes.iter().enumerate() {
                if r.app == a {
                    out.push((r.path.as_ref().map(|p| format!("{}{}", own, p)), i));
                }
            }
            for (parent, router, pfx) in &self.fa_includes {
                if *parent == a {
                    work.push((*router, format!("{}{}", own, pfx), depth + 1));
                }
            }
        }
        out
    }

    /// The arguments FastAPI passes `func` (a handler or a dependency) for request `req`
    /// (None: an entry run, every input open).
    pub fn fa_bind(&mut self, func: ObjId, req: Option<&Req>, state: &mut State, node: &Node, depth: u32) -> IndexMap<StrId, Value> {
        let fi = self.func_info(func);
        let mut out: IndexMap<StrId, Value> = IndexMap::new();
        let params: &'static ast::Parameters = match &fi.node {
            FuncNode::Def(f) => &f.parameters,
            _ => return out,
        };
        let skip_self = fi.cls.is_some() && !fi.is_static.get();
        let all: Vec<&'static ast::ParameterWithDefault> = params.posonlyargs.iter().chain(params.args.iter()).chain(params.kwonlyargs.iter()).collect();
        let body_params: Vec<String> = all.iter().filter(|p| self.fa_is_body_param(&fi, p, state)).map(|p| p.parameter.name.id.to_string()).collect();
        for (i, p) in all.iter().enumerate() {
            if i == 0 && skip_self {
                continue;
            }
            let pname = p.parameter.name.id.to_string();
            let v = self.fa_param(func, p, req, &body_params, state, node, depth);
            out.insert(s(&pname), v);
        }
        out
    }

    fn fa_marker(&self, fi: &crate::interp::FuncInfo, pname: &str) -> Option<(String, Value)> {
        let d = fi.defaults.get(&s(pname))?;
        let o = d.single()?;
        payload_of(self, o)
    }

    /// The metadata of `Annotated[T, Depends(g)]`: the marker objects, and T.
    fn fa_annotated(&mut self, fi: &crate::interp::FuncInfo, ann: &'static ast::Expr, state: &mut State) -> (Option<(String, Value)>, &'static ast::Expr) {
        if let ast::Expr::Subscript(sb) = ann {
            if dotted_last(&sb.value) == "Annotated" {
                if let ast::Expr::Tuple(t) = &*sb.slice {
                    let inner: &'static ast::Expr = &t.elts[0];
                    for m in t.elts.iter().skip(1) {
                        let m: &'static ast::Expr = m;
                        let v = self.eval_in_module(fi.module, m, state);
                        if let Some(o) = v.single() {
                            if let Some(p) = payload_of(self, o) {
                                return (Some(p), inner);
                            }
                        }
                    }
                    return (None, inner);
                }
            }
        }
        (None, ann)
    }

    fn fa_is_body_param(&mut self, fi: &crate::interp::FuncInfo, p: &'static ast::ParameterWithDefault, state: &mut State) -> bool {
        let pname = p.parameter.name.id.to_string();
        let (meta, ann) = match &p.parameter.annotation {
            Some(a) => {
                let (m, inner) = self.fa_annotated(fi, a, state);
                (m, Some(inner))
            }
            None => (None, None),
        };
        let marker = meta.or_else(|| self.fa_marker(fi, &pname));
        if let Some((q, _)) = &marker {
            return q.starts_with("fastapi.param.Body");
        }
        match ann {
            Some(a) => {
                let pa = self.pyd_ann_pub(a, fi.module, state);
                matches!(pa, Ann::Model(_) | Ann::List(_) | Ann::Optional(_)) && !matches!(pa, Ann::Optional(ref x) if matches!(**x, Ann::Other)) || matches!(dotted_last(a).as_str(), "dict" | "Dict") || matches!(a, ast::Expr::Subscript(sb) if matches!(dotted_last(&sb.value).as_str(), "dict" | "Dict" | "list" | "List"))
            }
            None => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fa_param(&mut self, func: ObjId, p: &'static ast::ParameterWithDefault, req: Option<&Req>, body_params: &[String], state: &mut State, node: &Node, depth: u32) -> Value {
        let fi = self.func_info(func);
        let pname = p.parameter.name.id.to_string();
        let (meta, ann) = match &p.parameter.annotation {
            Some(a) => {
                let (m, inner) = self.fa_annotated(&fi, a, state);
                (m, Some(inner))
            }
            None => (None, None),
        };
        let marker = meta.or_else(|| self.fa_marker(&fi, &pname));
        let ann_name = ann.map(|a| dotted_last(a)).unwrap_or_default();
        let open = |an: &mut Analyzer, st: &mut State| -> Value {
            Value::obj(an.unknown_source(&format!("param {}.{}", fi.qualname, pname), SourceKind::Param, st, &format!("param {}", pname), "", Ns::Item, vec![]))
        };
        // a dependency: its result
        if let Some((q, g)) = &marker {
            if q == "fastapi.depends" {
                let g = if g.empty() {
                    // `Depends()`: the annotation's class
                    match ann {
                        Some(a) => self.eval_in_module(fi.module, a, state),
                        None => Value::new(),
                    }
                } else {
                    g.clone()
                };
                return self.fa_call_dep(&g, req, state, node, depth + 1);
            }
        }
        // the request, background tasks, a response
        match ann_name.as_str() {
            "Request" | "WebSocket" | "HTTPConnection" => {
                let body = match req.and_then(|r| r.json.clone()) {
                    Some(j) => j,
                    None if req.is_some() => Value::data("body"),
                    None => {
                        let src = Value::obj(self.unknown_source(&format!("param {}.{}", fi.qualname, pname), SourceKind::Param, state, &format!("{}.json()", pname), "", Ns::Item, vec![]));
                        src
                    }
                };
                return fa_obj(self, state, node, "fastapi.request", body, &format!("req{}", func));
            }
            "BackgroundTasks" => return fa_obj(self, state, node, "fastapi.bg", Value::new(), &format!("bg{}", func)),
            "Response" => return Value::data("Response"),
            _ => {}
        }
        let route_path: Vec<(String, String)> = req.map(|r| r.path.clone()).unwrap_or_default();
        let is_body = body_params.contains(&pname);
        let default: Option<Value> = match &marker {
            Some((q, d)) if q.starts_with("fastapi.param.") => {
                if d.empty() {
                    None
                } else {
                    Some(d.clone())
                }
            }
            Some(_) => None,
            None => fi.defaults.get(&s(&pname)).cloned(),
        };
        let pann = ann.map(|a| self.pyd_ann_pub(a, fi.module, state)).unwrap_or(Ann::Other);
        if is_body {
            let embed = matches!(&marker, Some((q, _)) if q.ends_with(".embed")) || body_params.len() > 1;
            let raw = match req {
                Some(r) => match &r.json {
                    Some(j) if embed => {
                        let k = Value::lit(key_str_lit(&pname));
                        self.load_item(j, Ns::Item, &k, node, state, false)
                    }
                    Some(j) => j.clone(),
                    None => Value::data("body"),
                },
                None => open(self, state),
            };
            let report = req.is_none();
            let v = match &pann {
                Ann::Model(c) => self.pyd_validate_opts(*c, &raw, state, node, report),
                Ann::Other => raw,
                other => self.pyd_convert_pub(other, &raw, state, node),
            };
            return match default {
                Some(d) if req.is_none() => v.join(&d),
                _ => v,
            };
        }
        // a path / query / header / cookie value
        let mut v = Value::new();
        if let Some(r) = req {
            if let Some((_, val)) = route_path.iter().find(|(n, _)| *n == pname) {
                v = conv_scalar(&ann_name, val);
            } else if matches!(&marker, Some((q, _)) if q.starts_with("fastapi.param.Header")) {
                let hk = pname.replace('_', "-");
                if let Some(h) = &r.headers {
                    for o in h.pts.iter() {
                        let row = state.row(o, Ns::Item);
                        for k in row.present.iter() {
                            if intern::key_as_str(k).map(|x| x.to_lowercase() == hk.to_lowercase()).unwrap_or(false) {
                                v = v.join(&state.slot(o, Ns::Item, k));
                            }
                        }
                    }
                }
                if v.empty() {
                    v = default.clone().unwrap_or_else(|| Value::data("header"));
                }
            } else {
                let alias = match &marker {
                    Some((q, _)) if q.contains(".alias=") => q.split(".alias=").nth(1).unwrap_or(&pname).to_string(),
                    _ => pname.clone(),
                };
                if let Some((_, val)) = r.url_query.iter().find(|(n, _)| *n == alias) {
                    v = conv_scalar(&ann_name, val);
                } else if let Some(qd) = &r.query {
                    let k = key_str_lit(&alias);
                    let mut found = false;
                    for o in qd.pts.iter() {
                        let row = state.row(o, Ns::Item);
                        if row.present.contains(k) {
                            found = true;
                            let x = state.slot(o, Ns::Item, k);
                            v = v.join(&match str_of(&x) {
                                Some(t) => conv_scalar(&ann_name, &t),
                                None => x,
                            });
                            if !row.definite.contains(k) {
                                v = v.join(&default.clone().unwrap_or_else(|| Value::data("query")));
                            }
                        } else if row.open() {
                            found = true;
                            v = v.join(&Value::data("query"));
                        }
                    }
                    if !found {
                        v = default.clone().unwrap_or_else(|| Value::data("query"));
                    }
                } else {
                    v = default.clone().unwrap_or_else(|| Value::data("query"));
                }
            }
            return v;
        }
        // an entry run: anything the client sends, or the default
        let ov = open(self, state);
        match default {
            Some(d) => d.join(&ov),
            None => ov,
        }
    }

    /// A dependency `g` called by FastAPI: g's parameters bound like a handler's, a `yield`
    /// dependency's value is what it yields, a class is constructed.
    fn fa_call_dep(&mut self, g: &Value, req: Option<&Req>, state: &mut State, node: &Node, depth: u32) -> Value {
        if depth > 8 {
            return Value::data("depends");
        }
        let mut outs: Vec<Value> = Vec::new();
        for o in g.pts.iter() {
            match self.kind(o) {
                Kind::Function => {
                    let kwargs = self.fa_bind(o, req, state, node, depth);
                    let fi = self.func_info(o);
                    let r = self.call_user(o, None, vec![], kwargs, vec![], vec![], state, node, None, false);
                    let is_gen = match &fi.node {
                        FuncNode::Def(f) => crate::interp::is_generator(&f.body),
                        _ => false,
                    };
                    if is_gen {
                        let it = self.iterate(&r, node, state);
                        outs.push(match (it.items, it.elem) {
                            (Some(items), _) if !items.is_empty() => items[0].clone(),
                            (_, crate::interp::funcs::Elem::One(e)) => e,
                            _ => Value::data("depends"),
                        });
                    } else {
                        outs.push(r);
                    }
                }
                Kind::Class => {
                    let kwargs = match self.class_lookup(o, "__init__").and_then(|c| self.cls_slots.get(&(c, Ns::Attr, key_str_lit("__init__"))).cloned()) {
                        Some(fv) => match fv.pts.iter().find(|f| self.kind(*f) == Kind::Function) {
                            Some(init) => self.fa_bind(init, req, state, node, depth),
                            None => IndexMap::new(),
                        },
                        None => IndexMap::new(),
                    };
                    outs.push(self.construct(o, &[], &kwargs, &[], &[], state, node));
                }
                _ => outs.push(self.call_value(&Value::obj(o), node, None, state, Some(vec![]))),
            }
        }
        if outs.is_empty() {
            return Value::data("depends");
        }
        join_values(outs)
    }

    /// A route's handler called with a concrete request: an `HTTPException` (any exception the
    /// handler raises) ends in a response, so the call continues on those paths too.
    fn fa_dispatch(&mut self, route: usize, req: &Req, state: &mut State, node: &Node) -> Value {
        let r = self.fa_routes[route].clone();
        let Some(func) = r.func else { return Value::data("response") };
        let mut pre = state.copy();
        self.guards.push(rustc_hash::FxHashSet::from_iter(["HTTPException".to_string(), "RequestValidationError".to_string(), "ValidationError".to_string()]));
        self.may_raise.push(false);
        self.raise_states.push(None);
        self.raise_values.push(None);
        self.try_marks.push((self.frames.len(), self.env_stack.len(), pre.copy()));
        self.try_sum_depth.push(self.sum_stack.len());
        let kwargs = self.fa_bind(func, Some(req), state, node, 0);
        let ret = self.call_user(func, None, vec![], kwargs, vec![], vec![], state, node, None, false);
        self.guards.pop();
        self.try_marks.pop();
        self.try_sum_depth.pop();
        let flagged = self.may_raise.pop().unwrap_or(false);
        let raised_at = self.raise_states.pop().flatten();
        self.raise_values.pop();
        let raised = flagged || raised_at.is_some() || !state.reachable;
        let mut out = if state.reachable { self.fa_response_value(&r, func, &ret, state, node) } else { Value::new() };
        if raised {
            // the error response: {"detail": …}
            let did = self.alloc_at(Kind::Dict, node, state, &format!("error@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("faerr"));
            let dk = key_str_lit("detail");
            state.set_row(did, Ns::Item, Row::new().store(dk, true));
            state.set_slot(did, Ns::Item, dk, Value::data("detail"), true);
            out = out.join(&Value::obj(did));
            let mut after = state.copy();
            if flagged || (!state.reachable && raised_at.is_none()) {
                after = if after.reachable { pre.join(&mut after) } else { pre.copy() };
            }
            if let Some(mut rs) = raised_at {
                after = if after.reachable { after.join(&mut rs) } else { rs };
            }
            state.replace_with(after);
        }
        out
    }

    /// The JSON FastAPI sends for a handler's return value.
    fn fa_response_value(&mut self, r: &FaRoute, func: ObjId, ret: &Value, state: &mut State, node: &Node) -> Value {
        // a JSONResponse: its content
        let mut vals: Vec<Value> = Vec::new();
        let mut plain = Value::with_kv(crate::domain::ObjSet::new(), ret.kv.clone());
        for o in ret.pts.iter() {
            match payload_of(self, o) {
                Some((q, v)) if q == "fastapi.jsonresponse" => vals.push(v),
                _ => plain = plain.join(&Value::obj(o)),
            }
        }
        // response_model= (or the return annotation)
        let mut ann: Option<Ann> = None;
        let mut opts = DumpOpts::default();
        if let Some(c) = r.call {
            if let Some(e) = kw(c, "response_model") {
                let e: &'static ast::Expr = e;
                ann = Some(self.pyd_ann_pub(e, r.module, state));
            }
            for (k, f) in [("response_model_exclude_none", 0), ("response_model_exclude_unset", 1), ("response_model_exclude_defaults", 2), ("response_model_by_alias", 3)] {
                if let Some(ast::Expr::BooleanLiteral(b)) = kw(c, k) {
                    match f {
                        0 => opts.exclude_none = b.value,
                        1 => opts.exclude_unset = b.value,
                        2 => opts.exclude_defaults = b.value,
                        _ => {}
                    }
                }
            }
        }
        opts.by_alias = !matches!(r.call.and_then(|c| kw(c, "response_model_by_alias")), Some(ast::Expr::BooleanLiteral(b)) if !b.value);
        if ann.is_none() {
            let fi = self.func_info(func);
            if let FuncNode::Def(f) = &fi.node {
                if let Some(ra) = &f.returns {
                    let ra: &'static ast::Expr = ra;
                    ann = Some(self.pyd_ann_pub(ra, fi.module, state));
                }
            }
        }
        let ann = ann.unwrap_or(Ann::Other);
        let validated = match &ann {
            Ann::Model(c) => self.pyd_validate_opts(*c, &plain, state, node, false),
            Ann::Other => plain,
            other => self.pyd_convert_pub(other, &plain, state, node),
        };
        vals.push(self.pyd_dump_value_pub(&ann, &validated, &opts, state, node));
        join_values(vals)
    }

    /// `client.post(url, json=…, params=…, headers=…)`.
    fn fa_client_call(&mut self, app: &Value, method: &str, c: &Call, state: &mut State) -> Value {
        let url = c.arg(0, Some("url")).and_then(|u| str_of(&u));
        let mut req = Req { json: c.kwargs.get(&s("json")).cloned(), query: c.kwargs.get(&s("params")).cloned(), headers: c.kwargs.get(&s("headers")).cloned(), ..Default::default() };
        let (path, qs) = match &url {
            Some(u) => match u.split_once('?') {
                Some((p, q)) => (Some(p.to_string()), q.to_string()),
                None => (Some(u.clone()), String::new()),
            },
            None => (None, String::new()),
        };
        for part in qs.split('&').filter(|x| !x.is_empty()) {
            let (k, v) = part.split_once('=').unwrap_or((part, ""));
            req.url_query.push((k.to_string(), v.to_string()));
        }
        let mut outs: Vec<Value> = Vec::new();
        let mut states: Vec<State> = Vec::new();
        let mut pre = state.copy();
        for a in app.pts.iter() {
            for (full, idx) in self.fa_routes_of(a) {
                if !self.fa_routes[idx].methods.iter().any(|m| m == method || m == "*") {
                    continue;
                }
                let params = match (&full, &path) {
                    (Some(t), Some(p)) => match match_path(t, p) {
                        Some(ps) => ps,
                        None => continue,
                    },
                    // an unknown URL: any route of the method
                    _ => Vec::new(),
                };
                let mut st = pre.copy();
                let mut rq = req.clone();
                rq.path = params;
                let v = self.fa_dispatch(idx, &rq, &mut st, &c.node);
                outs.push(v);
                states.push(st);
                if path.is_some() {
                    // the first matching route answers
                    break;
                }
            }
        }
        if states.is_empty() {
            // no route: a 404 / 405 response
            let did = self.alloc_at(Kind::Dict, &c.node, state, &format!("error@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("fa404"));
            let dk = key_str_lit("detail");
            state.set_row(did, Ns::Item, Row::new().store(dk, true));
            state.set_slot(did, Ns::Item, dk, Value::data("detail"), true);
            return fa_obj(self, state, &c.node, "fastapi.response", Value::obj(did), "faresp");
        }
        let mut out = states.remove(0);
        for mut st in states {
            out = out.join(&mut st);
        }
        state.replace_with(out);
        let body = join_values(outs);
        fa_obj(self, state, &c.node, "fastapi.response", body, "faresp")
    }
}

fn conv_scalar(ann: &str, text: &str) -> Value {
    match ann {
        "int" => match text.parse::<i64>() {
            Ok(n) => Value::lit(intern::key_int(n)),
            Err(_) => Value::data("int"),
        },
        "float" => Value::data("float"),
        "bool" => match text.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Value::lit(intern::key_bool(true)),
            "false" | "0" | "no" | "off" => Value::lit(intern::key_bool(false)),
            _ => Value::data("bool"),
        },
        "str" | "" => Value::lit_str(text),
        _ => Value::data("param"),
    }
}

/// `fastapi.*`: constructors, markers and the methods of apps, routers, requests, clients,
/// responses and background tasks (receiver: the object's payload).
pub fn m_fastapi(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let q = c.qual.as_str();
    let last = q.rsplit('.').next().unwrap_or("");
    // methods on our objects: `fastapi.<kind>.<method>`
    if let Some(recv) = c.recv.clone() {
        if let Some(o) = recv.single() {
            if let Some((kind, pv)) = payload_of(an, o) {
                return object_method(an, st, c, o, &kind, &pv, last);
            }
        }
    }
    match last {
        "FastAPI" | "APIRouter" => {
            let v = fa_obj(an, st, &c.node, "fastapi.app", Value::new(), "faapp");
            if let (Some(o), Some(p)) = (v.single(), c.kwargs.get(&s("prefix")).and_then(str_of)) {
                an.fa_prefix.insert(o, p);
            }
            v
        }
        "Depends" | "Security" => {
            let g = c.arg(0, Some("dependency")).unwrap_or_default();
            fa_obj(an, st, &c.node, "fastapi.depends", g, &format!("dep{}", c.site))
        }
        "Query" | "Path" | "Header" | "Cookie" | "Body" | "Form" | "File" => {
            let d = c.arg(0, Some("default")).filter(|v| !(v.pts.is_empty() && matches!(&v.kv, KeyVal::Data(_)) && v.kv.data_site().map(|x| intern::str_get(x).contains("Ellipsis")).unwrap_or(false))).unwrap_or_default();
            let d = if is_ellipsis_value(c) { Value::new() } else { d };
            let mut tag = format!("fastapi.param.{}", last);
            if last == "Body" && matches!(c.kwargs.get(&s("embed")).and_then(|v| v.kv.as_lit()), Some(k) if k == intern::key_bool(true)) {
                tag.push_str(".embed");
            }
            if let Some(a) = c.kwargs.get(&s("alias")).and_then(str_of) {
                tag = format!("{}.alias={}", tag, a);
            }
            fa_obj(an, st, &c.node, &tag, d, &format!("par{}", c.site))
        }
        "TestClient" => {
            let app = c.arg(0, Some("app")).unwrap_or_default();
            fa_obj(an, st, &c.node, "fastapi.client", app, "faclient")
        }
        "JSONResponse" | "ORJSONResponse" | "UJSONResponse" => {
            let content = c.arg(0, Some("content")).unwrap_or_else(|| an.none_v.clone());
            fa_obj(an, st, &c.node, "fastapi.jsonresponse", content, &format!("jr{}", c.site))
        }
        "jsonable_encoder" => {
            let v = c.arg(0, Some("obj")).unwrap_or_default();
            an.pyd_dump_value_pub(&Ann::Other, &v, &DumpOpts::default(), st, &c.node)
        }
        _ => Value::data(&format!("fastapi.{}", last)),
    }
}

fn is_ellipsis_value(c: &Call) -> bool {
    match c.call_ast.and_then(|a| a.arguments.args.first()) {
        Some(ast::Expr::EllipsisLiteral(_)) => true,
        _ => matches!(c.call_ast.and_then(|a| kw(a, "default")), Some(ast::Expr::EllipsisLiteral(_))),
    }
}

fn object_method(an: &mut Analyzer, st: &mut State, c: &Call, o: ObjId, kind: &str, pv: &Value, name: &str) -> Value {
    match (kind, name) {
        ("fastapi.app", m) if HTTP_METHODS.contains(&m) || m == "api_route" || m == "route" || m == "websocket" => {
            let path = c.arg(0, Some("path")).and_then(|v| str_of(&v));
            let methods: Vec<String> = if HTTP_METHODS.contains(&m) {
                vec![m.to_string()]
            } else {
                match c.kwargs.get(&s("methods")) {
                    Some(ms) => {
                        let it = an.iterate(ms, &c.node, st);
                        it.items.map(|xs| xs.iter().filter_map(str_of).map(|x| x.to_lowercase()).collect()).unwrap_or_else(|| vec!["*".into()])
                    }
                    None => vec!["get".into()],
                }
            };
            let idx = an.fa_routes.len();
            let module = an.frame().module;
            an.fa_routes.push(FaRoute { app: o, methods, path, func: None, call: c.call_ast, module });
            fa_obj(an, st, &c.node, &format!("fastapi.route.{}", idx), Value::new(), &format!("route{}", idx))
        }
        ("fastapi.app", "include_router") => {
            let prefix = c.kwargs.get(&s("prefix")).and_then(str_of).unwrap_or_default();
            if let Some(r) = c.arg(0, Some("router")) {
                for ro in r.pts.iter() {
                    an.fa_includes.push((o, ro, prefix.clone()));
                }
            }
            an.none_v.clone()
        }
        ("fastapi.app", "add_api_route") => {
            let path = c.arg(0, Some("path")).and_then(|v| str_of(&v));
            let f = c.arg(1, Some("endpoint")).unwrap_or_default();
            let methods: Vec<String> = match c.kwargs.get(&s("methods")) {
                Some(ms) => {
                    let it = an.iterate(ms, &c.node, st);
                    it.items.map(|xs| xs.iter().filter_map(str_of).map(|x| x.to_lowercase()).collect()).unwrap_or_else(|| vec!["*".into()])
                }
                None => vec!["get".into()],
            };
            let module = an.frame().module;
            for fo in f.pts.iter().filter(|x| an.kind(*x) == Kind::Function).collect::<Vec<_>>() {
                an.fa_routes.push(FaRoute { app: o, methods: methods.clone(), path: path.clone(), func: Some(fo), call: c.call_ast, module });
                an.fa_handlers.insert(fo);
            }
            an.none_v.clone()
        }
        // decorators that register a function the framework calls (identity)
        ("fastapi.app", "middleware" | "exception_handler" | "on_event") => fa_obj(an, st, &c.node, "fastapi.hook", Value::new(), &format!("hook{}", c.site)),
        ("fastapi.app", "dependency_overrides") => Value::data("overrides"),
        ("fastapi.app", _) => an.none_v.clone(),
        ("fastapi.request", "json" | "body" | "form") => pv.clone(),
        ("fastapi.request", "query_params" | "headers" | "cookies" | "path_params") => {
            let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("request.{}", name), Payload::None, None, Ns::Item, None, Some(&format!("rq{}{}", o, name)));
            st.set_row(oid, Ns::Item, Row::with_source(Source::new(&format!("request.{}@L{}", name, c.line), SourceKind::External)));
            Value::obj(oid)
        }
        ("fastapi.bg", "add_task") => {
            // the task runs after the response: its arguments as given
            let f = c.args.first().cloned().unwrap_or_default();
            let args: Vec<Value> = c.args.iter().skip(1).cloned().collect();
            for fo in f.pts.iter().collect::<Vec<_>>() {
                if an.kind(fo) == Kind::Function {
                    an.call_user(fo, None, args.clone(), c.kwargs.clone(), vec![], vec![], st, &c.node, None, false);
                }
            }
            an.none_v.clone()
        }
        ("fastapi.client", m) if HTTP_METHODS.contains(&m) || m == "request" => {
            let (method, c2) = if m == "request" {
                let mm = c.arg(0, Some("method")).and_then(|v| str_of(&v)).map(|x| x.to_lowercase()).unwrap_or_else(|| "*".into());
                let mut c2 = c.clone();
                if !c2.args.is_empty() {
                    c2.args.remove(0);
                }
                (mm, c2)
            } else {
                (m.to_string(), c.clone())
            };
            an.fa_client_call(pv, &method, &c2, st)
        }
        ("fastapi.client", "__enter__") => Value::obj(o),
        ("fastapi.response", "json") => pv.clone(),
        ("fastapi.response", "headers") => {
            let oid = an.alloc_at(Kind::Dict, &c.node, st, "headers", Payload::None, None, Ns::Item, None, Some("rh"));
            st.set_row(oid, Ns::Item, Row::with_source(Source::new(&format!("response.headers@L{}", c.line), SourceKind::External)));
            Value::obj(oid)
        }
        ("fastapi.response", "raise_for_status") => an.none_v.clone(),
        _ => Value::data(&format!("{}.{}", kind, name)),
    }
}

/// Attribute values of our objects that are not methods (`response.status_code`,
/// `request.query_params`, `app.routes`).
pub fn object_attr(an: &mut Analyzer, oid: ObjId, name: &str, node: &Node, state: &mut State) -> Option<Value> {
    let (kind, pv) = payload_of(an, oid)?;
    let _ = pv;
    match (kind.as_str(), name) {
        ("fastapi.response", "status_code") => Some(Value::data("status_code")),
        ("fastapi.response", "text" | "content" | "reason_phrase" | "url") => Some(Value::data(name)),
        ("fastapi.response", "headers") | ("fastapi.request", "query_params" | "headers" | "cookies" | "path_params") => {
            let c = Call { qual: format!("{}.{}", kind, name), recv: Some(Value::obj(oid)), args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
            Some(m_fastapi(an, state, &c))
        }
        ("fastapi.request", "method" | "url" | "client" | "state" | "scope" | "app") => Some(Value::data(name)),
        ("fastapi.app", "state" | "routes" | "dependency_overrides" | "title" | "version") => Some(Value::data(name)),
        _ => None,
    }
}

/// A decorator expression whose callee is one of our objects' methods (`@app.get("/x")`):
/// the route decorator registers `f` as its handler.
pub fn apply_route(an: &mut Analyzer, dv: &Value, f: &Value) {
    for o in dv.pts.iter() {
        if let Some((q, _)) = payload_of(an, o) {
            if let Some(idx) = q.strip_prefix("fastapi.route.").and_then(|x| x.parse::<usize>().ok()) {
                for fo in f.pts.iter() {
                    if an.kind(fo) == Kind::Function {
                        an.fa_routes[idx].func = Some(fo);
                        an.fa_handlers.insert(fo);
                    }
                }
            }
        }
    }
}

pub fn install(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["fastapi.*", "starlette.*"], m_fastapi);
}
