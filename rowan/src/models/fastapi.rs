//! FastAPI: apps and routers record their routes; a handler is an entry
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
use crate::intern::{self, key_str_lit, s, KeyId, StrId};
use crate::interp::{Analyzer, Call, FuncNode, Payload};
use crate::models::pydantic::{Ann, DumpOpts};
use crate::state::{Kind, State};
use crate::syntax::dotted_last;

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
    /// the cookies the client sends (its jar, or `cookies=`)
    pub cookies: Option<Value>,
    pub path: Vec<(String, String)>,
    /// the request object (shared by the middlewares and the handler)
    pub obj: Option<Value>,
    /// any request (an entry run through the middlewares): values stay open
    pub open: bool,
}

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

fn fa_obj(an: &mut Analyzer, st: &mut State, node: &Node, qual: &str, v: Value, ctx: &str) -> Value {
    let oid = an.alloc_at(Kind::Namespace, node, st, qual, Payload::Method(qual.to_string(), v.clone()), Some(Row::new()), Ns::Attr, None, Some(ctx));
    an.heap.obj_mut(oid).payload = Payload::Method(qual.to_string(), v);
    // its attributes as slots (read through a placeholder in a summary like any object's)
    let attrs: &[&str] = match qual {
        "fastapi.request" => &["query_params", "headers", "cookies", "path_params", "method", "url", "client", "state", "scope", "app", "json", "body", "form"],
        "fastapi.response" => &["json", "status_code", "headers", "text", "content", "raise_for_status"],
        "fastapi.bg" => &["add_task"],
        "fastapi.client" => &["cookies"],
        "fastapi.outresp" => &["status_code", "headers", "set_cookie", "delete_cookie", "media_type", "body", "background"],
        _ => &[],
    };
    if !attrs.is_empty() {
        let mut row = Row::new();
        for a in attrs {
            let v = match object_attr(an, oid, a, node, st) {
                Some(v) => v,
                None => an.method_value(&format!("{}.{}", qual, a), &Value::obj(oid)),
            };
            let k = key_str_lit(a);
            row = row.store(k, true);
            st.set_slot(oid, Ns::Attr, k, v, true);
        }
        st.set_row(oid, Ns::Attr, row);
    }
    Value::obj(oid)
}

fn payload_of(an: &Analyzer, oid: ObjId) -> Option<(String, Value)> {
    match &an.heap.obj(oid).payload {
        Payload::Method(q, v) if q.starts_with("fastapi.") && an.heap.kind(oid) == Kind::Namespace => Some((q.clone(), v.clone())),
        _ => None,
    }
}

fn set_status(st: &mut State, resp: &Value, status: Value) {
    if let Some(o) = resp.single() {
        st.set_slot(o, Ns::Attr, key_str_lit("status_code"), status, true);
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

/// The name an exception class value stands for (`NotFound`, `KeyError`).
fn exc_name(an: &Analyzer, v: &Value) -> String {
    for o in v.pts.iter() {
        match an.heap.kind(o) {
            Kind::Class => return an.heap.class_info(o).name.clone(),
            Kind::External => return an.heap.external_qual(o).rsplit('.').next().unwrap_or("").to_string(),
            _ => {}
        }
    }
    String::new()
}

pub(crate) fn eval_in_module(an: &mut Analyzer, module: ObjId, e: &'static ast::Expr, state: &mut State) -> Value {
    an.in_module(module, "fastapi", "<fastapi>", |an| an.eval(e, state))
}

/// All routes of `app`: its own and those of the routers included in it (with prefixes),
/// as (full path, route index).
pub fn fa_routes_of(an: &Analyzer, app: ObjId) -> Vec<(Option<String>, usize)> {
    let mut out = Vec::new();
    let mut work: Vec<(ObjId, String, u32)> = vec![(app, String::new(), 0)];
    while let Some((a, prefix, depth)) = work.pop() {
        if depth > 8 {
            continue;
        }
        let own = format!("{}{}", prefix, an.m.fa_prefix.get(&a).cloned().unwrap_or_default());
        for (i, r) in an.m.fa_routes.iter().enumerate() {
            if r.app == a {
                out.push((r.path.as_ref().map(|p| format!("{}{}", own, p)), i));
            }
        }
        for (parent, router, pfx) in &an.m.fa_includes {
            if *parent == a {
                work.push((*router, format!("{}{}", own, pfx), depth + 1));
            }
        }
    }
    out
}

/// The arguments FastAPI passes `func` (a handler or a dependency) for request `req`
/// (None: an entry run, every input open).
pub fn fa_bind(an: &mut Analyzer, func: ObjId, req: Option<&Req>, state: &mut State, node: &Node, depth: u32) -> IndexMap<StrId, Value> {
    let fi = an.heap.func_info(func);
    let mut out: IndexMap<StrId, Value> = IndexMap::new();
    let params: &'static ast::Parameters = match &fi.node {
        FuncNode::Def(f) => &f.parameters,
        _ => return out,
    };
    let skip_self = fi.cls.is_some() && !fi.is_static.get();
    let all: Vec<&'static ast::ParameterWithDefault> = params.posonlyargs.iter().chain(params.args.iter()).chain(params.kwonlyargs.iter()).collect();
    let body_params: Vec<String> = all.iter().filter(|p| fa_is_body_param(an, &fi, p, state)).map(|p| p.parameter.name.id.to_string()).collect();
    // FastAPI solves the dependencies before it validates the request's own values (a
    // missing header is a 422 after the sub-dependencies ran)
    // (read off the program text: evaluating the markers again would make them weak)
    let is_dep = |an: &mut Analyzer, p: &'static ast::ParameterWithDefault, _st: &mut State| -> bool {
        let pname = p.parameter.name.id.to_string();
        let call_is_dep = |e: &ast::Expr| matches!(e, ast::Expr::Call(c) if matches!(dotted_last(&c.func).as_str(), "Depends" | "Security"));
        let in_ann = match p.parameter.annotation.as_deref() {
            Some(ast::Expr::Subscript(sb)) if dotted_last(&sb.value) == "Annotated" => match &*sb.slice {
                ast::Expr::Tuple(tu) => tu.elts.iter().skip(1).any(|e| call_is_dep(e)),
                _ => false,
            },
            _ => false,
        };
        in_ann || matches!(fa_marker(an, &fi, &pname), Some((q, _)) if q == "fastapi.depends")
    };
    let mut vals: Vec<Option<Value>> = vec![None; all.len()];
    for pass in 0..2 {
        for (i, p) in all.iter().enumerate() {
            if i == 0 && skip_self {
                continue;
            }
            let dep = false || is_dep(an, p, state);
            if (pass == 0) != dep {
                continue;
            }
            if pass == 1 && vals[i].is_some() {
                continue;
            }
            vals[i] = Some(fa_param(an, func, p, req, &body_params, state, node, depth));
        }
    }
    for (i, p) in all.iter().enumerate() {
        if let Some(v) = vals[i].take() {
            out.insert(s(p.parameter.name.id.as_str()), v);
        }
    }
    out
}

fn fa_marker(an: &Analyzer, fi: &crate::interp::FuncInfo, pname: &str) -> Option<(String, Value)> {
    let d = fi.defaults.get(&s(pname))?;
    let o = d.single()?;
    payload_of(an, o)
}

/// The metadata of `Annotated[T, Depends(g)]`: the marker objects, and T.
fn fa_annotated(an: &mut Analyzer, fi: &crate::interp::FuncInfo, ann: &'static ast::Expr, state: &mut State) -> (Option<(String, Value)>, &'static ast::Expr) {
    if let ast::Expr::Subscript(sb) = ann {
        if dotted_last(&sb.value) == "Annotated" {
            if let ast::Expr::Tuple(t) = &*sb.slice {
                let inner: &'static ast::Expr = &t.elts[0];
                for m in t.elts.iter().skip(1) {
                    let m: &'static ast::Expr = m;
                    let v = eval_in_module(an, fi.module, m, state);
                    if let Some(o) = v.single() {
                        if let Some(p) = payload_of(an, o) {
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

fn fa_is_body_param(an: &mut Analyzer, fi: &crate::interp::FuncInfo, p: &'static ast::ParameterWithDefault, state: &mut State) -> bool {
    let pname = p.parameter.name.id.to_string();
    let (meta, ann) = match &p.parameter.annotation {
        Some(a) => {
            let (m, inner) = fa_annotated(an, fi, a, state);
            (m, Some(inner))
        }
        None => (None, None),
    };
    let marker = meta.or_else(|| fa_marker(an, fi, &pname));
    if let Some((q, _)) = &marker {
        return q.starts_with("fastapi.param.Body");
    }
    match ann {
        Some(a) => {
            let pa = crate::models::pydantic::pyd_ann_pub(an, a, fi.module, state);
            bodyish(&pa) || matches!(dotted_last(a).as_str(), "dict" | "Dict") || matches!(a, ast::Expr::Subscript(sb) if matches!(dotted_last(&sb.value).as_str(), "dict" | "Dict"))
        }
        None => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn fa_param(an: &mut Analyzer, func: ObjId, p: &'static ast::ParameterWithDefault, req: Option<&Req>, body_params: &[String], state: &mut State, node: &Node, depth: u32) -> Value {
    let fi = an.heap.func_info(func);
    let dep_req = req;
    let shared_obj = req.and_then(|r| r.obj.clone());
    // an open request is any request: the values are those of an entry run
    let req = req.filter(|r| !r.open);
    let pname = p.parameter.name.id.to_string();
    let (meta, ann) = match &p.parameter.annotation {
        Some(a) => {
            let (m, inner) = fa_annotated(an, &fi, a, state);
            (m, Some(inner))
        }
        None => (None, None),
    };
    let marker = meta.or_else(|| fa_marker(an, &fi, &pname));
    let ann_name = ann.map(scalar_ann_name).unwrap_or_default();
    let open = |an: &mut Analyzer, st: &mut State| -> Value {
        Value::obj(an.unknown_source(&format!("param {}.{}", fi.py_qualname(), pname), SourceKind::Param, st, &format!("param {}", pname), "", Ns::Item, vec![]))
    };
    // a dependency: its result
    if let Some((q, g)) = &marker {
        if q == "fastapi.depends" {
            let g = if g.empty() {
                // `Depends()`: the annotation's class
                match ann {
                    Some(a) => eval_in_module(an, fi.module, a, state),
                    None => Value::new(),
                }
            } else {
                g.clone()
            };
            return fa_call_dep(an, &g, dep_req, state, node, depth + 1);
        }
    }
    // the request, background tasks, a response
    match ann_name.as_str() {
        "Request" | "WebSocket" | "HTTPConnection" => {
            if let Some(o) = shared_obj {
                return o;
            }
            let body = match req.and_then(|r| r.json.clone()) {
                Some(j) => j,
                None if req.is_some() => Value::data("body"),
                None => {
                    let src = Value::obj(an.unknown_source(&format!("param {}.{}", fi.py_qualname(), pname), SourceKind::Param, state, &format!("{}.json()", pname), "", Ns::Item, vec![]));
                    src
                }
            };
            return fa_obj(an, state, node, "fastapi.request", body, &format!("req{}", func));
        }
        "BackgroundTasks" => return fa_obj(an, state, node, "fastapi.bg", Value::new(), &format!("bg{}", func)),
        "Response" => {
            let v = fa_obj(an, state, node, "fastapi.outresp", Value::new(), &format!("resp{}", func));
            if let (Some(o), Some(top)) = (v.single(), an.m.fa_out_resp.last_mut()) {
                top.push(o);
            }
            return v;
        }
        _ => {}
    }
    let route_path: Vec<(String, String)> = req.map(|r| r.path.clone()).unwrap_or_default();
    let is_body = body_params.contains(&pname);
    let default: Option<Value> = match &marker {
        Some((q, d)) if q.starts_with("fastapi.param.") => {
            if d.empty() {
                // (`Annotated[int, Query(ge=1)] = 2`: the parameter's own default)
                fi.defaults.get(&s(&pname)).cloned().filter(|dv| !dv.pts.iter().any(|o| payload_of(an, o).is_some()))
            } else {
                Some(d.clone())
            }
        }
        Some(_) => None,
        None => fi.defaults.get(&s(&pname)).cloned(),
    };
    let pann = ann.map(|a| crate::models::pydantic::pyd_ann_pub(an, a, fi.module, state)).unwrap_or(Ann::Other);
    if is_body {
        let embed = matches!(&marker, Some((q, _)) if q.ends_with(".embed")) || body_params.len() > 1;
        let raw = match req {
            Some(r) => match &r.json {
                Some(j) if embed => {
                    // the key in the JSON, else the default
                    let kk = key_str_lit(&pname);
                    let recs: Vec<ObjId> = j.pts.iter().filter(|o| matches!(an.heap.kind(*o), Kind::Dict | Kind::Unknown)).collect();
                    let sure = !recs.is_empty() && recs.iter().all(|o| state.row(*o, Ns::Item).definite.contains(kk));
                    let maybe = recs.iter().any(|o| state.row(*o, Ns::Item).present.contains(kk) || state.row(*o, Ns::Item).open());
                    let mut v = if maybe { an.load_item(j, Ns::Item, &Value::lit(kk), node, state, false) } else { Value::new() };
                    if !sure {
                        match &default {
                            Some(d) => v = v.join(d),
                            // a required body the JSON does not hold under its name: 422
                            None if !maybe => {
                                an.raise_here("RequestValidationError", state);
                                return Value::new();
                            }
                            None => an.may_raise_here("RequestValidationError", state),
                        }
                    }
                    v
                }
                Some(j) => j.clone(),
                None => Value::data("body"),
            },
            None => open(an, state),
        };
        let report = req.is_none();
        let v = match &pann {
            Ann::Model(c) => {
                // (a test request's body: its validation errors are the 422 response)
                let saved = std::mem::replace(&mut an.m.fa_body_validation, !report);
                let v = crate::models::pydantic::pyd_validate_opts(an, *c, &raw, state, node, report);
                an.m.fa_body_validation = saved;
                v
            }
            Ann::Other => raw,
            other => crate::models::pydantic::pyd_convert_pub(an, other, &raw, state, node),
        };
        return match default {
            Some(d) if req.is_none() => v.join(&d),
            _ => v,
        };
    }
    // a path / query / header / cookie value
    let mut v = Value::new();
    // (an Enum / Literal annotation validates the text itself)
    let restrictable = ann.map(|a| fa_restrictable(an, &fi, a, state)).unwrap_or(false);
    // (a text the annotation surely rejects converts to nothing and is a 422 below)
    let bad = std::cell::Cell::new(false);
    let conv = |ann_name: &str, text: &str| {
        let v = if restrictable { Value::lit_str(text) } else { conv_scalar(ann_name, text) };
        if v.empty() {
            bad.set(true);
        }
        v
    };
    if let Some(r) = req {
        if let Some((_, val)) = route_path.iter().find(|(n, _)| *n == pname) {
            v = conv(&ann_name, val);
        } else if matches!(&marker, Some((q, _)) if q.starts_with("fastapi.param.Cookie")) {
            let ck = match &marker {
                Some((q, _)) if q.contains(".alias=") => q.split(".alias=").nth(1).unwrap_or(&pname).to_string(),
                _ => pname.clone(),
            };
            let mut found = false;
            if let Some(cj) = &r.cookies {
                for o in cj.pts.iter() {
                    let row = state.row(o, Ns::Item);
                    let k = key_str_lit(&ck);
                    if row.present.contains(k) {
                        v = v.join(&state.slot(o, Ns::Item, k));
                        found = row.definite.contains(k);
                        if !found {
                            v = v.join(&default.clone().unwrap_or_else(|| Value::data("cookie")));
                        }
                        found = true;
                    } else if row.open() {
                        v = v.join(&Value::data("cookie"));
                        found = true;
                    }
                }
            }
            if !found {
                v = default.clone().unwrap_or_else(|| Value::data("cookie"));
            }
        } else if matches!(&marker, Some((q, _)) if q.starts_with("fastapi.param.Header")) {
            let hk = pname.replace('_', "-");
            // (whether the request surely has it / surely lacks it)
            let mut surely_absent = true;
            let mut surely_present = false;
            if let Some(h) = &r.headers {
                if !h.kv.is_bot() && !h.kv.is_literal() {
                    surely_absent = false;
                }
                for o in h.pts.iter() {
                    let row = state.row(o, Ns::Item);
                    if row.open() {
                        surely_absent = false;
                    }
                    for k in row.present.iter() {
                        if intern::key_as_str(k).map(|x| x.to_lowercase() == hk.to_lowercase()).unwrap_or(false) {
                            v = v.join(&state.slot(o, Ns::Item, k));
                            surely_absent = false;
                            surely_present |= row.definite.contains(k) && h.pts.len() == 1;
                        }
                    }
                }
            }
            // (the headers a TestClient always sends)
            const CLIENT_HEADERS: &[(&str, &str)] = &[("host", "testserver"), ("accept", "*/*"), ("accept-encoding", "gzip, deflate"), ("connection", "keep-alive"), ("user-agent", "testclient")];
            if !surely_present {
                if let Some((_, hv)) = CLIENT_HEADERS.iter().find(|(n, _)| *n == hk.to_lowercase()) {
                    if v.empty() {
                        v = Value::lit_str(hv);
                    }
                    surely_absent = false;
                }
            }
            if v.empty() || !surely_present {
                match default.clone() {
                    Some(d) => v = v.join(&d),
                    None if surely_absent && v.empty() => {
                        // a required header the request does not send: 422
                        an.raise_here("RequestValidationError", state);
                        return Value::new();
                    }
                    None if !surely_present => {
                        an.may_raise_here("RequestValidationError", state);
                        if v.empty() {
                            v = Value::data("header");
                        }
                    }
                    None => {
                        if v.empty() {
                            v = Value::data("header");
                        }
                    }
                }
            }
        } else {
            let alias = match &marker {
                Some((q, _)) if q.contains(".alias=") => q.split(".alias=").nth(1).unwrap_or(&pname).to_string(),
                _ => pname.clone(),
            };
            if let Some((_, val)) = r.url_query.iter().find(|(n, _)| *n == alias) {
                v = conv(&ann_name, val);
            } else if let Some(qd) = &r.query {
                let k = key_str_lit(&alias);
                let mut found = false;
                for o in qd.pts.iter() {
                    let row = state.row(o, Ns::Item);
                    if row.present.contains(k) {
                        found = true;
                        let x = state.slot(o, Ns::Item, k);
                        v = v.join(&match str_of(&x) {
                            Some(t) => conv(&ann_name, &t),
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
        if bad.get() {
            if v.empty() {
                an.raise_here("RequestValidationError", state);
                return Value::new();
            }
            an.may_raise_here("RequestValidationError", state);
        }
        // `Query(ge=1)` / `Path(gt=0)` constraints on the value the request gave (a default
        // is not validated)
        let cons = marker_constraints(p);
        if !cons.is_empty() && default.as_ref() != Some(&v) {
            if v.pts.is_empty() && v.kv.is_literal() {
                let lits = crate::domain::kv_literals(&v.kv);
                let bad: Vec<KeyId> = lits.iter().filter(|k| *k != intern::key_none() && crate::models::pydantic::literal_breaks(&cons, *k).is_some()).collect();
                if !bad.is_empty() {
                    if bad.len() == lits.len() {
                        an.raise_here("RequestValidationError", state);
                        return Value::new();
                    }
                    an.may_raise_here("RequestValidationError", state);
                    v = Value::kv_only(crate::domain::KeyVal::lits(intern::KeySet::from_iter(lits.iter().filter(|k| !bad.contains(k)))));
                }
            } else if !v.empty() {
                an.may_raise_here("RequestValidationError", state);
            }
        }
        if let Some(a) = ann {
            v = fa_restrict(an, &fi, a, v, default.as_ref(), state);
        }
        return v;
    }
    // an entry run: anything the client sends, or the default
    let mut ov = open(an, state);
    if let Some(a) = ann {
        ov = fa_restrict(an, &fi, a, ov, None, state);
    }
    match default {
        Some(d) => d.join(&ov),
        None => ov,
    }
}

fn fa_restrictable(an: &mut Analyzer, fi: &crate::interp::FuncInfo, ann: &'static ast::Expr, state: &mut State) -> bool {
    match ann {
        ast::Expr::Subscript(sb) => dotted_last(&sb.value) == "Literal",
        ast::Expr::Name(_) | ast::Expr::Attribute(_) => {
            let cv = eval_in_module(an, fi.module, ann, state);
            cv.single().map(|c| an.heap.kind(c) == Kind::Class && an.m.enum_members.contains_key(&c)).unwrap_or(false)
        }
        _ => false,
    }
}

/// A path / query value validated by its annotation: an Enum class makes it the member with
/// that value, `Literal[...]` one of the literals; anything else is a 422 (raised here).
fn fa_restrict(an: &mut Analyzer, fi: &crate::interp::FuncInfo, ann: &'static ast::Expr, v: Value, default: Option<&Value>, state: &mut State) -> Value {
    // (the default is not validated)
    if let Some(d) = default {
        if v == *d {
            return v;
        }
    }
    let allowed: Vec<(Value, Option<KeyId>)> = if let ast::Expr::Subscript(sb) = ann {
        if dotted_last(&sb.value) != "Literal" {
            return v;
        }
        let elts: Vec<&'static ast::Expr> = match &*sb.slice {
            ast::Expr::Tuple(t) => t.elts.iter().collect(),
            e => vec![e],
        };
        let mut out = Vec::new();
        for e in elts {
            let lv = eval_in_module(an, fi.module, e, state);
            match lv.kv.as_lit().filter(|_| lv.pts.is_empty()) {
                Some(k) => out.push((lv.clone(), Some(k))),
                None => return v,
            }
        }
        out
    } else {
        let cv = eval_in_module(an, fi.module, ann, state);
        let Some(cls) = cv.single().filter(|c| an.heap.kind(*c) == Kind::Class && an.m.enum_members.contains_key(c)) else { return v };
        let vk = key_str_lit("value");
        an.m.enum_members[&cls].clone().iter().map(|(_, m)| (Value::obj(*m), state.slot(*m, Ns::Attr, vk).kv.as_lit())).collect()
    };
    if allowed.is_empty() {
        return v;
    }
    // the text the client sent: compared with each allowed value's str()
    let given = if v.pts.is_empty() && v.kv.is_literal() { Some(crate::domain::kv_literals(&v.kv)) } else { None };
    match given {
        Some(ks) => {
            let mut out = Value::new();
            let mut miss = false;
            for k in ks.iter() {
                let text = intern::key_py_str(k);
                match allowed.iter().find(|(_, a)| a.map(|a| a == k || intern::key_py_str(a) == text).unwrap_or(false)) {
                    Some((av, _)) => out = out.join(av),
                    None => miss = true,
                }
            }
            if out.empty() {
                an.raise_here("RequestValidationError", state);
                return Value::new();
            }
            if miss {
                an.may_raise_here("RequestValidationError", state);
            }
            out
        }
        None => {
            an.may_raise_here("RequestValidationError", state);
            join_values(allowed.into_iter().map(|(a, _)| a))
        }
    }
}

/// A dependency `g` called by FastAPI: g's parameters bound like a handler's, a `yield`
/// dependency's value is what it yields, a class is constructed.
fn fa_call_dep(an: &mut Analyzer, g: &Value, req: Option<&Req>, state: &mut State, node: &Node, depth: u32) -> Value {
    if depth > 8 {
        return Value::data("depends");
    }
    // a dependency runs once per request: its value is reused
    if let Some(v) = g.single().and_then(|o| an.m.fa_dep_cache.last().and_then(|c| c.get(&o).cloned())) {
        return v;
    }
    let mut outs: Vec<Value> = Vec::new();
    for o in g.pts.iter() {
        match an.heap.kind(o) {
            Kind::Function => {
                let kwargs = fa_bind(an, o, req, state, node, depth);
                let fi = an.heap.func_info(o);
                let r = an.call_user(o, None, vec![], kwargs, vec![], vec![], state, node, false);
                let is_gen = match &fi.node {
                    FuncNode::Def(f) => crate::syntax::is_generator(&f.body),
                    _ => false,
                };
                if is_gen {
                    let it = an.iterate(&r, node, state);
                    outs.push(match (it.items, it.elem) {
                        (Some(items), _) if !items.is_empty() => items[0].clone(),
                        (_, crate::interp::funcs::Elem::One(e)) => e,
                        _ => Value::data("depends"),
                    });
                    // (FastAPI runs it to its end after the response: its effects all happen)
                    an.exhaust(&r, state);
                } else {
                    outs.push(r);
                }
            }
            Kind::Class => {
                let kwargs = match an.class_lookup(o, "__init__").and_then(|c| an.heap.cls_slots.get(&(c, Ns::Attr, key_str_lit("__init__"))).cloned()) {
                    Some(fv) => match fv.pts.iter().find(|f| an.heap.kind(*f) == Kind::Function) {
                        Some(init) => fa_bind(an, init, req, state, node, depth),
                        None => IndexMap::new(),
                    },
                    None => IndexMap::new(),
                };
                outs.push(an.construct(o, &[], &kwargs, &[], &[], state, node));
            }
            _ => outs.push(an.call_value(&Value::obj(o), node, None, state, Some(vec![]))),
        }
    }
    if outs.is_empty() {
        return Value::data("depends");
    }
    let v = join_values(outs);
    if let (Some(o), Some(c)) = (g.single(), an.m.fa_dep_cache.last_mut()) {
        c.insert(o, v.clone());
    }
    v
}

/// The handler's `Depends(g)` parameters whose `g` is a generator function (`yield` dependencies).
pub fn fa_yield_deps(an: &mut Analyzer, func: ObjId, state: &mut State) -> Vec<ObjId> {
    let fi = an.heap.func_info(func);
    let FuncNode::Def(f) = &fi.node else { return vec![] };
    let mut out = Vec::new();
    for p in f.parameters.posonlyargs.iter().chain(f.parameters.args.iter()).chain(f.parameters.kwonlyargs.iter()) {
        let pname = p.parameter.name.id.to_string();
        let meta = match &p.parameter.annotation {
            Some(a) => fa_annotated(an, &fi, a, state).0,
            None => None,
        };
        if let Some((q, g)) = meta.or_else(|| fa_marker(an, &fi, &pname)) {
            if q != "fastapi.depends" {
                continue;
            }
            if let Some(o) = g.single().filter(|o| an.heap.kind(*o) == Kind::Function) {
                let gi = an.heap.func_info(o);
                if matches!(&gi.node, FuncNode::Def(d) if crate::syntax::is_generator(&d.body)) && !out.contains(&o) {
                    out.push(o);
                }
            }
        }
    }
    out
}

/// The handler called inside its yield dependencies: each runs to its `yield`, the rest of
/// the request (the next one, finally the handler) runs there, then its code after `yield`.
pub fn fa_run_handler(an: &mut Analyzer, func: ObjId, mut gens: Vec<ObjId>, req: Option<Req>, entry: bool, state: &mut State, node: &Node) -> Value {
    if gens.is_empty() {
        let kwargs = fa_bind(an, func, req.as_ref(), state, node, 0);
        return an.call_user(func, None, vec![], kwargs, vec![], vec![], state, node, entry);
    }
    let g = gens.remove(0);
    let kwargs = fa_bind(an, g, req.as_ref(), state, node, 1);
    let nd = *node;
    let then: Box<dyn FnOnce(&mut Analyzer, Value, &mut State)> = Box::new(move |an: &mut Analyzer, v: Value, st: &mut State| {
        if let Some(c) = an.m.fa_dep_cache.last_mut() {
            c.insert(g, v);
        }
        let r = fa_run_handler(an, func, gens, req, entry, st, &nd);
        an.m.fa_gen_ret = Some(an.m.fa_gen_ret.take().unwrap_or_default().join(&r));
    });
    let outer = an.m.fa_gen_ret.take();
    let h = an.with_cm_then(then, |an| {
        an.call_user(g, None, vec![], kwargs, vec![], vec![], state, node, false);
    });
    let ret = std::mem::replace(&mut an.m.fa_gen_ret, outer).unwrap_or_default();
    match h {
        Some(h) if h.ran => {
            if let Some(env) = h.env_after {
                state.env = env;
            }
            ret
        }
        // the dependency never reached its yield: no handler
        _ => Value::new(),
    }
}

/// The entry run of a handler when middlewares are registered: any request, through the
/// first middleware (its `call_next` runs the handler). None without middlewares.
pub fn fa_entry_through_middleware(an: &mut Analyzer, func: ObjId, state: &mut State, node: &Node) -> Option<()> {
    let mws: Vec<ObjId> = an.m.fa_hooks.iter().filter(|h| h.1 == "mw").filter_map(|h| h.3).collect();
    let first = *mws.first()?;
    let idx = an.m.fa_routes.iter().position(|r| r.func == Some(func))?;
    let fi = an.heap.func_info(func);
    let body = Value::obj(an.unknown_source(&format!("param {}.request", fi.py_qualname()), SourceKind::Param, state, "request.json()", "", Ns::Item, vec![]));
    let robj = fa_obj(an, state, node, "fastapi.request", body, &format!("entryreq{}", func));
    let rq = Req { obj: Some(robj.clone()), open: true, ..Default::default() };
    let n = an.m.fa_pending.len();
    an.m.fa_pending.push((idx, rq));
    let cn = an.heap.alloc_raw(Kind::Method, &format!("callnext:{}", n), "", true, "call_next", Payload::Method(format!("fastapi.callnext.{}", n), Value::new()));
    an.call_user(first, None, vec![robj, Value::obj(cn)], IndexMap::new(), vec![], vec![], state, node, false);
    Some(())
}

/// The dependencies a route runs before its handler's parameters: those of the apps /
/// routers it belongs to (outermost first), then `dependencies=[…]` of the route.
pub fn fa_route_deps(an: &mut Analyzer, idx: usize, req: Option<&Req>, state: &mut State, node: &Node) {
    let r = an.m.fa_routes[idx].clone();
    // the chain of apps / routers above the route's
    let mut chain: Vec<ObjId> = vec![r.app];
    let mut cur = r.app;
    for _ in 0..8 {
        match an.m.fa_includes.iter().find(|(_, ro, _)| *ro == cur) {
            Some((parent, _, _)) => {
                chain.push(*parent);
                cur = *parent;
            }
            None => break,
        }
    }
    let mut lists: Vec<Value> = Vec::new();
    for a in chain.iter().rev() {
        lists.extend(an.m.fa_app_deps.get(a).cloned().unwrap_or_default());
    }
    if let Some(e) = r.call.and_then(|c| kw(c, "dependencies")) {
        let e: &'static ast::Expr = e;
        lists.push(eval_in_module(an, r.module, e, state));
    }
    for l in lists {
        let it = an.iterate(&l, node, state);
        let markers: Vec<Value> = match it.items {
            Some(items) => items,
            None => vec![an.elem_value(&it.elem, node, "el")],
        };
        for m in markers {
            for o in m.pts.iter() {
                if let Some((q, g)) = payload_of(an, o) {
                    if q == "fastapi.depends" {
                        fa_call_dep(an, &g, req, state, node, 1);
                    }
                }
            }
        }
    }
}

/// A route's handler called with a concrete request: an `HTTPException` (any exception the
/// handler raises) ends in a response, so the call continues on those paths too.
fn fa_dispatch(an: &mut Analyzer, route: usize, req: &Req, state: &mut State, node: &Node) -> Value {
    let r = an.m.fa_routes[route].clone();
    let Some(func) = r.func else { return Value::data("response") };
    let mut pre = state.copy();
    // exception handlers (`@app.exception_handler(E)`): E is caught too
    let hooks: Vec<(String, Value, ObjId)> = an.m.fa_hooks.iter().filter(|h| h.1 == "exc").filter_map(|h| h.3.map(|f| (exc_name(an, &h.2), h.2.clone(), f))).collect();
    // (a pydantic ValidationError the handler's own code raises is no request error: the
    // TestClient re-raises it, as any other exception FastAPI does not answer)
    let mut caught: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::from_iter(["HTTPException".to_string(), "RequestValidationError".to_string()]);
    for (n, _, _) in &hooks {
        caught.insert(n.clone());
    }
    let caught_names = crate::models::exceptions::expand_caught(caught);
    an.enter_try(caught_names.clone(), &mut pre);
    an.m.fa_dep_cache.push(rustc_hash::FxHashMap::default());
    an.m.fa_out_resp.push(Vec::new());
    fa_route_deps(an, route, Some(req), state, node);
    let gens = fa_yield_deps(an, func, state);
    let ret = fa_run_handler(an, func, gens, Some(req.clone()), false, state, node);
    let out_resps = an.m.fa_out_resp.pop().unwrap_or_default();
    an.m.fa_dep_cache.pop();
    let out_try = an.leave_try();
    let mut raised_v = out_try.value();
    let recs = out_try.recs;
    // (the raises are kept one by one: their states joined)
    let mut raised_at: Option<State> = crate::interp::join_raise_recs(&recs);
    // exceptions of known classes that FastAPI does not answer (a KeyError, …) are
    // re-raised by the TestClient at the call: they go on to the enclosing try
    if !recs.is_empty() {
        let answered = |c: &Option<crate::interp::ExcNames>| match c {
            Some((names, _)) => names.iter().any(|n| caught_names.contains(n)),
            None => true,
        };
        if recs.iter().any(|(c, _, _)| !answered(c)) {
            let mut keep_st: Option<State> = None;
            let mut keep_v = Value::new();
            for (c, st, ev) in recs {
                if answered(&c) {
                    let mut s2 = st.copy();
                    keep_st = Some(match keep_st { None => s2, Some(mut a) => a.join(&mut s2) });
                    keep_v = keep_v.join(&ev);
                } else {
                    an.reraise_rec((c, st, ev));
                }
            }
            raised_at = keep_st;
            raised_v = keep_v;
        }
    }
    let mut out = if state.reachable { fa_response_value(an, &r, func, &ret, state, node) } else { Value::new() };
    // the status: a returned JSONResponse's, else one the handler set on its Response, else
    // the route's
    let sk = key_str_lit("status_code");
    let mut status = Value::new();
    if state.reachable {
        let mut plain = !ret.kv.is_bot();
        for o in ret.pts.iter() {
            match payload_of(an, o) {
                Some((q, _)) if q == "fastapi.jsonresponse" => status = status.join(&state.slot(o, Ns::Attr, sk)),
                _ => plain = true,
            }
        }
        if ret.empty() {
            plain = true;
        }
        if plain {
            let set: Vec<Value> = out_resps.iter().map(|o| state.slot(*o, Ns::Attr, sk)).collect();
            let unset = |v: &Value| v.empty() || matches!(&v.kv, KeyVal::Data(d) if intern::str_get(*d).contains("unset_status"));
            let mut route_default = set.is_empty();
            for v in &set {
                if unset(v) {
                    route_default = true;
                } else {
                    // (a value set on some paths only: joined with the unset marker)
                    if let KeyVal::Lits(_) | KeyVal::Lit(_) = v.kv {
                        status = status.join(v);
                    } else {
                        status = status.join(&Value::data("status_code"));
                        route_default = true;
                    }
                }
            }
            if route_default {
                status = status.join(&fa_route_status(an, &r, state));
            }
        }
    }
    if let Some(mut rs) = raised_at {
        // the raised exceptions: a registered handler's response, else (an HTTPException, a
        // validation error) the error response {"detail": …}
        let mut unmatched = raised_v.pts.is_empty() || !raised_v.kv.is_bot();
        let mut rest: Vec<ObjId> = Vec::new();
        for o in raised_v.pts.iter() {
            let cls = if an.heap.kind(o) == Kind::Instance { an.heap.instance_class(o) } else { None };
            let httpexc = payload_of(an, o).map(|(q, _)| q == "fastapi.httpexc").unwrap_or(false);
            let hit = hooks.iter().find(|(n, hv, _)| match cls {
                Some(c) => hv.pts.iter().any(|h| an.heap.kind(h) == Kind::Class && an.heap.mro(c).contains(&h)) || an.heap.mro(c).iter().any(|m| an.heap.class_info_opt(*m).map(|ci| ci.name == *n).unwrap_or(false)),
                None => n == "KeyError" && an.heap.obj(o).label.starts_with("KeyError") || n == "Exception" || httpexc && n == "HTTPException",
            }).cloned();
            match hit {
                Some((_, _, hf)) => {
                    let robj = req.obj.clone().unwrap_or_else(|| Value::data("request"));
                    let resp = an.call_user(hf, None, vec![robj, Value::obj(o)], IndexMap::new(), vec![], vec![], &mut rs, node, false);
                    let mut rv: Vec<Value> = Vec::new();
                    for x in resp.pts.iter() {
                        if let Some((q, v)) = payload_of(an, x) {
                            if q == "fastapi.jsonresponse" {
                                rv.push(v);
                                status = status.join(&rs.slot(x, Ns::Attr, sk));
                            }
                        }
                    }
                    out = out.join(&join_values(rv));
                }
                None => rest.push(o),
            }
        }
        if !rest.is_empty() {
            unmatched = true;
        }
        if unmatched {
            // the detail of a raised HTTPException
            let mut detail = Value::new();
            let mut other = raised_v.pts.is_empty() || !raised_v.kv.is_bot();
            // (a request body that breaks its model: FastAPI's detail is pydantic's errors,
            // their `loc` after "body")
            let mut verr = false;
            for o in &rest {
                let verr_idx = match &an.heap.obj(*o).payload {
                    Payload::Method(q, _) => q.strip_prefix("pydverr.").and_then(|x| x.parse::<usize>().ok()),
                    _ => None,
                };
                if let Some(i) = verr_idx {
                    let errs = crate::models::pydantic::verr_errors(an, &mut rs, node, i, false, Some(key_str_lit("body")));
                    detail = detail.join(&errs);
                    status = status.join(&Value::lit(intern::key_int(422)));
                    verr = true;
                    continue;
                }
                match payload_of(an, *o) {
                    Some((q, _)) if q == "fastapi.httpexc" => {
                        detail = detail.join(&rs.slot(*o, Ns::Attr, key_str_lit("detail")));
                        status = status.join(&rs.slot(*o, Ns::Attr, sk));
                    }
                    _ => other = true,
                }
            }
            let _ = verr;
            // (a validation error: 422)
            if other {
                status = status.join(&Value::lit(intern::key_int(422)));
            }
            if other || detail.empty() {
                detail = detail.join(&Value::data("detail"));
            }
            let did = an.alloc_at(Kind::Dict, node, state, &format!("error@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("faerr"));
            let dk = key_str_lit("detail");
            rs.set_row(did, Ns::Item, Row::new().store(dk, true));
            rs.set_slot(did, Ns::Item, dk, detail, true);
            out = out.join(&Value::obj(did));
        }
        let mut after = if state.reachable { state.copy().join(&mut rs) } else { rs.copy() };
        // (an error response is an exceptional exit: what the handler needs on its normal
        // path stays needed — `always`)
        if state.reachable {
            after.reqs = state.reqs.join_with(&rs.reqs, &mut |a, b| a.join(b), &mut |a| a.clone());
        }
        state.replace_with(after);
    }
    an.m.fa_status = if status.empty() { Value::data("status_code") } else { status };
    out
}

/// A route's `status_code=` (a literal or `status.HTTP_201_CREATED`), else 200.
fn fa_route_status(an: &mut Analyzer, r: &FaRoute, state: &mut State) -> Value {
    let Some(e) = r.call.and_then(|c| kw(c, "status_code")) else { return Value::lit(intern::key_int(200)) };
    if let ast::Expr::Attribute(at) = e {
        let n = at.attr.as_str();
        if let Some(code) = n.strip_prefix("HTTP_").and_then(|x| x.get(..3)).and_then(|x| x.parse::<i64>().ok()) {
            return Value::lit(intern::key_int(code));
        }
    }
    let e: &'static ast::Expr = e;
    let v = eval_in_module(an, r.module, e, state);
    if v.pts.is_empty() && v.kv.as_lit().and_then(intern::key_as_int).is_some() {
        v
    } else {
        Value::data("status_code")
    }
}

/// The JSON FastAPI sends for a handler's return value.
fn fa_response_value(an: &mut Analyzer, r: &FaRoute, func: ObjId, ret: &Value, state: &mut State, node: &Node) -> Value {
    // a JSONResponse: its content
    let mut vals: Vec<Value> = Vec::new();
    let mut plain = Value::with_kv(crate::domain::ObjSet::new(), ret.kv.clone());
    for o in ret.pts.iter() {
        match payload_of(an, o) {
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
            ann = Some(crate::models::pydantic::pyd_ann_pub(an, e, r.module, state));
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
    // response_model_include= / response_model_exclude= (literal sets)
    for (k, inc) in [("response_model_include", true), ("response_model_exclude", false)] {
        if let Some(e) = r.call.and_then(|c| kw(c, k)) {
            let e: &'static ast::Expr = e;
            let v = eval_in_module(an, r.module, e, state);
            match crate::models::pydantic::key_spec(an, state, &v, node) {
                Some((ks, _)) => {
                    if inc {
                        opts.include = Some(ks);
                    } else {
                        opts.exclude = Some(ks);
                    }
                }
                None => opts.unknown_sel = true,
            }
        }
    }
    if ann.is_none() {
        let fi = an.heap.func_info(func);
        if let FuncNode::Def(f) = &fi.node {
            if let Some(ra) = &f.returns {
                let ra: &'static ast::Expr = ra;
                ann = Some(crate::models::pydantic::pyd_ann_pub(an, ra, fi.module, state));
            }
        }
    }
    let ann = ann.unwrap_or(Ann::Other);
    // (a response model serialises in pydantic's JSON mode; plain data goes through
    // jsonable_encoder: Decimals as floats)
    let decimal_float = matches!(ann, Ann::Other);
    opts.json = !decimal_float;
    // a returned model instance (of a subclass, say) goes through the response model's
    // fields: dumped, then validated as the response model
    if !matches!(ann, Ann::Other) {
        plain = crate::models::pydantic::pyd_dump_value_pub(an, &Ann::Other, &plain, &DumpOpts::default(), state, node);
    }
    let validated = match &ann {
        Ann::Model(c) => crate::models::pydantic::pyd_validate_opts(an, *c, &plain, state, node, false),
        Ann::Other => plain,
        other => crate::models::pydantic::pyd_convert_pub(an, other, &plain, state, node),
    };
    let dumped = crate::models::pydantic::pyd_dump_value_pub(an, &ann, &validated, &opts, state, node);
    vals.push(crate::models::json_wire(an, state, node, &dumped, decimal_float));
    join_values(vals)
}

/// The request object of a `TestClient` call: its method, headers (case-insensitive keys,
/// with the client's own), query and path parameters as the call gives them.
fn fa_fill_request(an: &mut Analyzer, robj: &Value, rq: &Req, method: &str, state: &mut State, node: &Node) {
    let Some(o) = robj.single() else { return };
    let lit_dict = |an: &mut Analyzer, st: &mut State, name: &str, items: Vec<(String, Value)>, ci: bool| -> Option<Value> {
        let d = an.alloc_at(Kind::Dict, node, st, &format!("request.{}", name), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("rq{}{}", o, name)));
        let mut row = Row::new();
        for (k, v) in items {
            let kk = key_str_lit(&if ci { k.to_lowercase() } else { k });
            row = row.store(kk, true);
            st.set_slot(d, Ns::Item, kk, v, true);
        }
        st.set_row(d, Ns::Item, row);
        if ci {
            an.m.ci_dicts.insert(d);
        }
        Some(Value::obj(d))
    };
    // the items of a literal dict argument (None: not known)
    let items_of = |an: &Analyzer, st: &State, v: &Option<Value>| -> Option<Vec<(String, Value)>> {
        let Some(v) = v else { return Some(vec![]) };
        let d = v.single().filter(|d| an.heap.kind(*d) == Kind::Dict)?;
        let row = st.row(d, Ns::Item);
        if !row.closed() || row.present != row.definite {
            return None;
        }
        Some(row.present.iter().filter_map(|k| intern::key_as_str(k).map(|x| (x.to_string(), st.slot(d, Ns::Item, k)))).collect())
    };
    if let Some(mut hs) = items_of(an, state, &rq.headers) {
        // httpx's own headers
        for (k, v) in [("host", "testserver"), ("accept", "*/*"), ("accept-encoding", "gzip, deflate"), ("connection", "keep-alive"), ("user-agent", "testclient")] {
            if !hs.iter().any(|(h, _)| h.to_lowercase() == k) {
                hs.push((k.to_string(), Value::lit_str(v)));
            }
        }
        if rq.json.is_some() {
            hs.push(("content-length".into(), Value::data("content-length")));
            hs.push(("content-type".into(), Value::lit_str("application/json")));
        }
        if let Some(v) = lit_dict(an, state, "headers", hs, true) {
            state.set_slot(o, Ns::Attr, key_str_lit("headers"), v, true);
        }
    }
    if let Some(mut qs) = items_of(an, state, &rq.query) {
        // (query values are strings)
        for q in qs.iter_mut() {
            if let Some(k) = q.1.kv.as_lit().filter(|_| q.1.pts.is_empty()) {
                q.1 = Value::lit_str(&intern::key_py_str(k));
            } else {
                q.1 = Value::data("query");
            }
        }
        for (k, v) in &rq.url_query {
            qs.push((k.clone(), Value::lit_str(v)));
        }
        if let Some(v) = lit_dict(an, state, "query_params", qs, false) {
            state.set_slot(o, Ns::Attr, key_str_lit("query_params"), v, true);
        }
    }
    let ps: Vec<(String, Value)> = rq.path.iter().map(|(k, v)| (k.clone(), Value::lit_str(v))).collect();
    if let Some(v) = lit_dict(an, state, "path_params", ps, false) {
        state.set_slot(o, Ns::Attr, key_str_lit("path_params"), v, true);
    }
    if method != "*" {
        state.set_slot(o, Ns::Attr, key_str_lit("method"), Value::lit_str(&method.to_uppercase()), true);
    }
}

/// `client.post(url, json=…, params=…, headers=…)`.
fn fa_client_call(an: &mut Analyzer, app: &Value, method: &str, c: &Call, state: &mut State) -> Value {
    let url = c.arg(0, Some("url")).and_then(|u| str_of(&u));
    let mut req = Req { json: c.kwargs.get(&s("json")).cloned(), query: c.kwargs.get(&s("params")).cloned(), headers: c.kwargs.get(&s("headers")).cloned(), ..Default::default() };
    // the cookies: `cookies=`, else the client's jar
    req.cookies = c.kwargs.get(&s("cookies")).cloned().or_else(|| {
        let cl = c.recv.as_ref().and_then(|r| r.single())?;
        state.slot_opt(cl, Ns::Attr, key_str_lit("cookies"))
    });
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
    let mut statuses: Vec<Value> = Vec::new();
    let pre = state.copy();
    for a in app.pts.iter() {
        for (full, idx) in fa_routes_of(an, a) {
            if !an.m.fa_routes[idx].methods.iter().any(|m| m == method || m == "*") {
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
            // one request object for the middlewares and the handler
            let body = rq.json.clone().unwrap_or_else(|| Value::data("concrete"));
            let robj = fa_obj(an, &mut st, &c.node, "fastapi.request", body, &format!("req{}", c.line));
            fa_fill_request(an, &robj, &rq, method, &mut st, &c.node);
            rq.obj = Some(robj.clone());
            let mws: Vec<ObjId> = an.m.fa_hooks.iter().filter(|h| h.1 == "mw").filter_map(|h| h.3).collect();
            let v = if mws.is_empty() {
                let b = fa_dispatch(an, idx, &rq, &mut st, &c.node);
                statuses.push(an.m.fa_status.clone());
                b
            } else {
                // the (first) middleware runs with call_next dispatching the route
                let n = an.m.fa_pending.len();
                an.m.fa_pending.push((idx, rq.clone()));
                let cn = an.heap.alloc_raw(Kind::Method, &format!("callnext:{}", n), "", true, "call_next", Payload::Method(format!("fastapi.callnext.{}", n), Value::new()));
                let resp = an.call_user(mws[0], None, vec![robj.clone(), Value::obj(cn)], IndexMap::new(), vec![], vec![], &mut st, &c.node, false);
                let mut bodies: Vec<Value> = Vec::new();
                let mut stv = Value::new();
                for x in resp.pts.iter() {
                    if let Some((q, v)) = payload_of(an, x) {
                        if q == "fastapi.response" || q == "fastapi.jsonresponse" {
                            bodies.push(v);
                            stv = stv.join(&st.slot(x, Ns::Attr, key_str_lit("status_code")));
                        }
                    }
                }
                statuses.push(if stv.empty() || resp.pts.len() != bodies.len() { Value::data("status_code") } else { stv });
                join_values(bodies)
            };
            outs.push(v);
            states.push(st);
            if path.is_some() {
                // the first matching route answers
                break;
            }
        }
    }
    if states.is_empty() {
        // no route: a 404 / 405 response (through the middlewares, which run for it too)
        let path_known = path.is_some();
        let other_method = app.pts.iter().any(|a| fa_routes_of(an, a).iter().any(|(full, _)| match (full, &path) {
            (Some(t), Some(p)) => match_path(t, p).is_some(),
            _ => false,
        }));
        let code = if !path_known { Value::data("status_code") } else { Value::lit(intern::key_int(if other_method { 405 } else { 404 })) };
        an.m.fa_status = code.clone();
        let did = an.alloc_at(Kind::Dict, &c.node, state, &format!("error@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("fa404"));
        let dk = key_str_lit("detail");
        state.set_row(did, Ns::Item, Row::new().store(dk, true));
        state.set_slot(did, Ns::Item, dk, Value::data("detail"), true);
        let mws: Vec<ObjId> = an.m.fa_hooks.iter().filter(|h| h.1 == "mw").filter_map(|h| h.3).collect();
        if let Some(first) = mws.first().copied() {
            let body = req.json.clone().unwrap_or_else(|| Value::data("concrete"));
            let robj = fa_obj(an, state, &c.node, "fastapi.request", body, &format!("req{}", c.line));
            fa_fill_request(an, &robj, &req, method, state, &c.node);
            let n = an.m.fa_pending.len();
            an.m.fa_pending.push((usize::MAX, Req { obj: Some(robj.clone()), json: Some(Value::obj(did)), ..Default::default() }));
            let cn = an.heap.alloc_raw(Kind::Method, &format!("callnext:{}", n), "", true, "call_next", Payload::Method(format!("fastapi.callnext.{}", n), Value::new()));
            an.call_user(first, None, vec![robj, Value::obj(cn)], IndexMap::new(), vec![], vec![], state, &c.node, false);
        }
        let v = fa_obj(an, state, &c.node, "fastapi.response", Value::obj(did), "faresp");
        set_status(state, &v, code);
        return v;
    }
    let mut out = states.remove(0);
    for mut st in states {
        out = out.join(&mut st);
    }
    state.replace_with(out);
    let body = join_values(outs);
    let v = fa_obj(an, state, &c.node, "fastapi.response", body, "faresp");
    set_status(state, &v, join_values(statuses));
    v
}

/// A type FastAPI reads from the body: a model, a TypedDict, a union of models, a dict, or a
/// list / Optional of those (scalars and lists of scalars are query parameters).
fn bodyish(a: &Ann) -> bool {
    match a {
        Ann::Model(_) | Ann::TypedDict(_) | Ann::Union(..) | Ann::Dict(_) => true,
        Ann::List(x, _) | Ann::Optional(x) => bodyish(x),
        _ => false,
    }
}

/// A path / query text as the scalar annotation `ann` converts it; nothing when the text is
/// surely rejected (a 422), unknown data when the conversion is not modelled.
fn conv_scalar(ann: &str, text: &str) -> Value {
    // (empty, or a character no number has, and not inf / infinity / nan)
    let surely_not_number = |t: &str| {
        let t = t.trim();
        let word = t.trim_start_matches(['+', '-']).to_ascii_lowercase();
        t.is_empty() || (t.chars().any(|ch| !(ch.is_ascii_digit() || "+-._eE".contains(ch))) && !matches!(word.as_str(), "inf" | "infinity" | "nan"))
    };
    match ann {
        "int" => match text.parse::<i64>() {
            Ok(n) => Value::lit(intern::key_int(n)),
            Err(_) if surely_not_number(text) => Value::new(),
            Err(_) => Value::data("int"),
        },
        "float" => match text.parse::<f64>() {
            Ok(f) => Value::lit(intern::key_float(f)),
            Err(_) if surely_not_number(text) => Value::new(),
            Err(_) => Value::data("float"),
        },
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
    // `client.cookies.set(name, value)`: a store into the jar
    if q == "fastapi.cookies.set" || q == "fastapi.cookies.delete" {
        let recv = c.recv.clone().unwrap_or_default();
        if let Some(k) = c.args.first() {
            if q.ends_with("set") {
                let v = c.args.get(1).cloned().or_else(|| c.kw("value")).unwrap_or_default();
                an.store_item(&recv, Ns::Item, k, &v, &c.node, st);
            } else {
                an.delete_item(&recv, Ns::Item, k, &c.node, st, false);
            }
        }
        return an.none_v.clone();
    }
    // a middleware's call_next(request): the route's response
    if let Some(n) = q.strip_prefix("fastapi.callnext.").and_then(|x| x.parse::<usize>().ok()) {
        let Some((idx, rq)) = an.m.fa_pending.get(n).cloned() else { return Value::data("response") };
        // (no route: the 404 body)
        let (body, status) = if idx == usize::MAX { (rq.json.clone().unwrap_or_default(), an.m.fa_status.clone()) } else { (fa_dispatch(an, idx, &rq, st, &c.node), an.m.fa_status.clone()) };
        let v = fa_obj(an, st, &c.node, "fastapi.response", body, &format!("mwresp{}", n));
        set_status(st, &v, status);
        return v;
    }
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
                an.m.fa_prefix.insert(o, p);
            }
            if let (Some(o), Some(d)) = (v.single(), c.kwargs.get(&s("dependencies")).cloned()) {
                an.m.fa_app_deps.entry(o).or_default().push(d);
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
        // `HTTPException(status_code, detail=None, headers=None)`: raised, it becomes the
        // response `{"detail": detail}`
        "HTTPException" => {
            let code = c.arg(0, Some("status_code")).unwrap_or_default();
            let detail = c.arg(1, Some("detail")).unwrap_or_else(|| Value::data("detail"));
            let headers = c.arg(2, Some("headers")).unwrap_or_else(|| an.none_v.clone());
            let v = fa_obj(an, st, &c.node, "fastapi.httpexc", detail.clone(), &format!("hx{}", c.site));
            if let Some(o) = v.single() {
                let mut row = Row::new();
                for (k, val) in [("status_code", code), ("detail", detail), ("headers", headers)] {
                    let kk = key_str_lit(k);
                    row = row.store(kk, true);
                    st.set_slot(o, Ns::Attr, kk, val, true);
                }
                st.set_row(o, Ns::Attr, row);
            }
            v
        }
        "JSONResponse" | "ORJSONResponse" | "UJSONResponse" => {
            let content = c.arg(0, Some("content")).unwrap_or_else(|| an.none_v.clone());
            let code = c.arg(1, Some("status_code")).unwrap_or_else(|| Value::lit(intern::key_int(200)));
            let v = fa_obj(an, st, &c.node, "fastapi.jsonresponse", content, &format!("jr{}", c.site));
            if let Some(o) = v.single() {
                let k = key_str_lit("status_code");
                st.set_slot(o, Ns::Attr, k, code, true);
                st.set_row(o, Ns::Attr, st.row(o, Ns::Attr).store(k, true));
            }
            v
        }
        "jsonable_encoder" => {
            let v = c.arg(0, Some("obj")).unwrap_or_default();
            crate::models::pydantic::pyd_dump_value_pub(an, &Ann::Other, &v, &DumpOpts::default(), st, &c.node)
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
            let idx = an.m.fa_routes.len();
            let module = an.frame().module;
            an.m.fa_routes.push(FaRoute { app: o, methods, path, func: None, call: c.call_ast, module });
            fa_obj(an, st, &c.node, &format!("fastapi.route.{}", idx), Value::new(), &format!("route{}", idx))
        }
        ("fastapi.app", "include_router") => {
            let prefix = c.kwargs.get(&s("prefix")).and_then(str_of).unwrap_or_default();
            if let Some(r) = c.arg(0, Some("router")) {
                for ro in r.pts.iter() {
                    an.m.fa_includes.push((o, ro, prefix.clone()));
                    if let Some(d) = c.kwargs.get(&s("dependencies")).cloned() {
                        an.m.fa_app_deps.entry(ro).or_default().push(d);
                    }
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
            for fo in f.pts.iter().filter(|x| an.heap.kind(*x) == Kind::Function).collect::<Vec<_>>() {
                an.m.fa_routes.push(FaRoute { app: o, methods: methods.clone(), path: path.clone(), func: Some(fo), call: c.call_ast, module });
                an.m.fa_handlers.insert(fo);
            }
            an.none_v.clone()
        }
        // decorators that register a function the framework calls (identity)
        ("fastapi.app", "exception_handler" | "middleware") => {
            let exc = if name == "exception_handler" { c.arg(0, Some("exc_class_or_status_code")).unwrap_or_default() } else { Value::new() };
            let idx = an.m.fa_hooks.len();
            an.m.fa_hooks.push((o, if name == "middleware" { "mw".into() } else { "exc".into() }, exc, None));
            fa_obj(an, st, &c.node, &format!("fastapi.hook.{}", idx), Value::new(), &format!("hook{}", idx))
        }
        ("fastapi.app", "on_event") => fa_obj(an, st, &c.node, "fastapi.hookx", Value::new(), &format!("hook{}", c.site)),
        ("fastapi.app", "dependency_overrides") => Value::data("overrides"),
        ("fastapi.app", _) => an.none_v.clone(),
        // (the raw body of a known json= payload: its bytes as httpx sends them)
        ("fastapi.request", "body") => match crate::models::json_body_text(an, st, &pv) {
            Some(t) => Value::lit(intern::key(intern::Key::Bytes(t.into_bytes()))),
            None => pv.clone(),
        },
        ("fastapi.request", "json" | "form") => pv.clone(),
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
                if an.heap.kind(fo) == Kind::Function {
                    an.call_user(fo, None, args.clone(), c.kwargs.clone(), vec![], vec![], st, &c.node, false);
                } else if c.kwargs.is_empty() {
                    // (a bound method, a builtin's method, a partial: called as well)
                    an.call_value(&Value::obj(fo), &c.node, None, st, Some(args.clone()));
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
            fa_client_call(an, pv, &method, &c2, st)
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
    match (kind.as_str(), name) {
        // the client's cookie jar: a dict (set / clear / get / update / [k] = v)
        ("fastapi.client", "cookies") => {
            let d = an.alloc_at(Kind::Dict, node, state, "cookies", Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("jar{}", oid)));
            state.set_row(d, Ns::Item, Row::new());
            an.m.cookie_jars.insert(d);
            Some(Value::obj(d))
        }
        ("fastapi.response", "status_code") => Some(Value::data("status_code")),
        // (not set by the handler: the route's status)
        ("fastapi.outresp", "status_code") => Some(Value::data("unset_status")),
        ("fastapi.outresp", "headers") => {
            let d = an.alloc_at(Kind::Dict, node, state, "response.headers", Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("orh{}", oid)));
            Some(Value::obj(d))
        }
        ("fastapi.outresp", "media_type" | "body" | "background") => Some(Value::data(name)),
        ("fastapi.response", "text" | "content" | "reason_phrase" | "url") => Some(Value::data(name)),
        ("fastapi.response", "headers") | ("fastapi.request", "query_params" | "headers" | "cookies" | "path_params") => {
            let c = Call { qual: format!("{}.{}", kind, name), recv: Some(Value::obj(oid)), args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
            Some(m_fastapi(an, state, &c))
        }
        ("fastapi.request", "state") => {
            // an object the middlewares and handlers set attributes on (open for any request)
            let sid = an.alloc_at(Kind::Namespace, node, state, "request.state", Payload::None, Some(Row::new()), Ns::Attr, None, Some(&format!("state{}", oid)));
            // a fresh object per request: what the middlewares / dependencies set on it
            let _ = pv;
            state.set_row(sid, Ns::Attr, Row::new());
            Some(Value::obj(sid))
        }
        ("fastapi.request", "method" | "url" | "client" | "scope" | "app") => Some(Value::data(name)),
        ("fastapi.app", "state" | "routes" | "dependency_overrides" | "title" | "version") => Some(Value::data(name)),
        _ => None,
    }
}

/// A decorator expression whose callee is one of our objects' methods (`@app.get("/x")`):
/// the route decorator registers `f` as its handler.
pub fn apply_route(an: &mut Analyzer, dv: &Value, f: &Value) {
    for o in dv.pts.iter() {
        if let Some((q, _)) = payload_of(an, o) {
            if let Some(idx) = q.strip_prefix("fastapi.hook.").and_then(|x| x.parse::<usize>().ok()) {
                for fo in f.pts.iter() {
                    if an.heap.kind(fo) == Kind::Function {
                        an.m.fa_hooks[idx].3 = Some(fo);
                        // (run around each handler, not on its own)
                        an.mark_entry_done(fo);
                    }
                }
            }
            if let Some(idx) = q.strip_prefix("fastapi.route.").and_then(|x| x.parse::<usize>().ok()) {
                for fo in f.pts.iter() {
                    if an.heap.kind(fo) == Kind::Function {
                        an.m.fa_routes[idx].func = Some(fo);
                        an.m.fa_handlers.insert(fo);
                    }
                }
            }
        }
    }
}

pub fn install(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["fastapi.*", "starlette.*"], m_fastapi);
}

/// The constraints of a parameter's `Query` / `Path` / `Header` / `Cookie` / `Form` marker
/// (in `Annotated[...]` or as the default).
fn marker_constraints(p: &'static ast::ParameterWithDefault) -> Vec<(String, KeyId)> {
    let mut calls: Vec<&'static ast::ExprCall> = Vec::new();
    if let Some(ast::Expr::Subscript(sb)) = p.parameter.annotation.as_deref() {
        if dotted_last(&sb.value) == "Annotated" {
            if let ast::Expr::Tuple(tp) = &*sb.slice {
                for m in tp.elts.iter().skip(1) {
                    if let ast::Expr::Call(c) = m {
                        calls.push(c);
                    }
                }
            }
        }
    }
    if let Some(ast::Expr::Call(c)) = p.default.as_deref() {
        calls.push(c);
    }
    calls.into_iter().filter(|c| matches!(dotted_last(&c.func).as_str(), "Query" | "Path" | "Header" | "Cookie" | "Form")).flat_map(crate::models::pydantic::field_constraints).collect()
}

/// The scalar a path / query annotation converts to: `Optional[X]`, `Union[X, None]` and
/// `X | None` are X.
fn scalar_ann_name(a: &ast::Expr) -> String {
    let is_none = |e: &ast::Expr| matches!(e, ast::Expr::NoneLiteral(_)) || dotted_last(e) == "None";
    match a {
        ast::Expr::Subscript(sb) if dotted_last(&sb.value) == "Optional" => scalar_ann_name(&sb.slice),
        ast::Expr::Subscript(sb) if dotted_last(&sb.value) == "Union" => match &*sb.slice {
            ast::Expr::Tuple(t) => {
                let rest: Vec<&ast::Expr> = t.elts.iter().filter(|e| !is_none(e)).collect();
                if rest.len() == 1 { scalar_ann_name(rest[0]) } else { dotted_last(a) }
            }
            _ => dotted_last(a),
        },
        ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr && is_none(&b.right) => scalar_ann_name(&b.left),
        ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr && is_none(&b.left) => scalar_ann_name(&b.right),
        _ => dotted_last(a),
    }
}

/// The entry run of a FastAPI handler: called by the framework with any request (also when
/// the program's own TestClient calls it), through its middlewares and dependencies, from `st`;
/// its requirements join `state`'s; `general`: its values hold for any request.
pub(crate) fn fa_entry_run(an: &mut Analyzer, oid: ObjId, fi: &crate::interp::FuncInfo, mut st: State, state: &mut State, general: bool) {
    an.as_entry_run(oid, fi, general, |an, fake| {
        if fa_entry_through_middleware(an, oid, &mut st, fake).is_none() {
            an.m.fa_dep_cache.push(rustc_hash::FxHashMap::default());
            if let Some(idx) = an.m.fa_routes.iter().position(|r| r.func == Some(oid)) {
                fa_route_deps(an, idx, None, &mut st, fake);
            }
            let gens = fa_yield_deps(an, oid, &mut st);
            fa_run_handler(an, oid, gens, None, true, &mut st, fake);
            an.m.fa_dep_cache.pop();
        }
    });
    for (src, k) in st.reqs.diff_keys(&state.reqs) {
        let Some(info) = st.reqs.get(&src, &k) else { continue };
        let nv = match state.reqs.get(&src, &k) {
            Some(cur) => cur.join(&info),
            None => info,
        };
        state.reqs.set(&src, k, nv);
    }
}
