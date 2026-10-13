//! functools: `singledispatch` (a call dispatches on its first argument's type to the
//! implementation registered for the nearest class) and `lru_cache` / `cache` (`memo_call`).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, Ns, ObjId, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, KeyId, StrId};
use crate::interp::{Analyzer, Payload};
use crate::state::{Kind, State};

impl Analyzer {
    /// The class a singledispatch implementation's first parameter is annotated with.
    pub fn sdispatch_annotation(&mut self, f: ObjId, method: bool, state: &mut State) -> Option<Value> {
        let fi = self.func_info(f);
        let crate::interp::FuncNode::Def(d) = &fi.node else { return None };
        let ps: Vec<&'static ast::ParameterWithDefault> = d.parameters.posonlyargs.iter().chain(d.parameters.args.iter()).collect();
        let p = ps.get(if method { 1 } else { 0 })?;
        let ann: &'static ast::Expr = p.parameter.annotation.as_deref()?;
        let mi = self.module_info(fi.module);
        self.frames.push(crate::interp::Frame::new(crate::interp::FrameKind::Module, mi.scopes.module(), fi.module, "sdispatch", true, "<ann>"));
        let v = self.eval(ann, state);
        self.frames.pop();
        Some(v)
    }

    /// The types a value may have, for dispatch: builtin type names / classes (None: unknown).
    fn dispatch_types(&self, v: &Value) -> Vec<(Option<Result<String, ObjId>>, Value)> {
        let mut out = Vec::new();
        for k in crate::domain::kv_literals(&v.kv).iter() {
            let t = if k == intern::key_none() {
                "NoneType".to_string()
            } else {
                match intern::key_get(k) {
                    intern::Key::Str(_) => "str".into(),
                    intern::Key::Int(_) => "int".into(),
                    intern::Key::Bool(_) => "bool".into(),
                    intern::Key::Float(_) => "float".into(),
                    intern::Key::Tuple(_) => "tuple".into(),
                    intern::Key::Bytes(_) => "bytes".into(),
                    _ => {
                        out.push((None, Value::lit(k)));
                        continue;
                    }
                }
            };
            out.push((Some(Ok(t)), Value::lit(k)));
        }
        if !v.kv.is_literal() && !v.kv.is_bot() {
            out.push((None, Value::kv_only(v.kv.clone())));
        }
        for o in v.pts.iter() {
            let t = match self.kind(o) {
                Kind::Dict if !self.m.counters.contains(&o) => Some(Ok("dict".to_string())),
                Kind::List if !self.m.deques.contains_key(&o) => Some(Ok("list".to_string())),
                Kind::Tuple => Some(Ok("tuple".to_string())),
                Kind::Set => Some(Ok("set".to_string())),
                Kind::Instance => self.instance_class(o).map(Err),
                _ => None,
            };
            out.push((t, Value::obj(o)));
        }
        out
    }

    /// Whether type `t` is (a subclass of) the registered type `reg`, and how far up (0: itself).
    fn dispatch_distance(&self, t: &Result<String, ObjId>, reg: &Value) -> Option<usize> {
        let mut best: Option<usize> = None;
        for r in reg.pts.iter() {
            let d = match (self.kind(r), t) {
                (Kind::External, Ok(name)) => {
                    let q = self.external_qual(r);
                    let rn = q.rsplit('.').next().unwrap_or("").to_string();
                    let chain: &[&str] = match name.as_str() {
                        "bool" => &["bool", "int", "object"],
                        "dict" => &["dict", "MutableMapping", "Mapping", "Collection", "Iterable", "object"],
                        "list" => &["list", "MutableSequence", "Sequence", "Collection", "Iterable", "object"],
                        "tuple" => &["tuple", "Sequence", "Collection", "Iterable", "object"],
                        "str" => &["str", "Sequence", "Collection", "Iterable", "object"],
                        "set" => &["set", "MutableSet", "Set", "AbstractSet", "Collection", "Iterable", "object"],
                        "int" => &["int", "Integral", "Real", "Number", "object"],
                        "float" => &["float", "Real", "Number", "object"],
                        _ => &[],
                    };
                    let pos = chain.iter().position(|c| *c == rn).or_else(|| if rn == name.as_str() { Some(0) } else { None });
                    pos
                }
                (Kind::External, Err(_)) => {
                    let q = self.external_qual(r);
                    if q.ends_with(".object") || q == "object" { Some(1000) } else { None }
                }
                (Kind::Class, Err(c)) => self.mro(*c).iter().position(|m| *m == r),
                _ => None,
            };
            if let Some(d) = d {
                best = Some(best.map(|b| b.min(d)).unwrap_or(d));
            }
        }
        best
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sdispatch_call(&mut self, foid: ObjId, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, entry: bool) -> Value {
        let reg = self.sdispatch.get(&foid).cloned().unwrap_or_default();
        let mut groups: Vec<(ObjId, Value)> = Vec::new();
        let add = |f: ObjId, v: Value, groups: &mut Vec<(ObjId, Value)>| match groups.iter_mut().find(|(g, _)| *g == f) {
            Some((_, cur)) => *cur = cur.join(&v),
            None => groups.push((f, v)),
        };
        for (t, part) in self.dispatch_types(&args[0]) {
            match t {
                Some(t) => {
                    let mut best: Option<(usize, ObjId)> = None;
                    for (ty, f) in &reg {
                        if let Some(d) = self.dispatch_distance(&t, ty) {
                            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                                best = Some((d, *f));
                            }
                        }
                    }
                    add(best.map(|b| b.1).unwrap_or(foid), part, &mut groups);
                }
                // an unknown type: any implementation
                None => {
                    for (_, f) in &reg {
                        add(*f, part.clone(), &mut groups);
                    }
                    add(foid, part, &mut groups);
                }
            }
        }
        if groups.is_empty() {
            groups.push((foid, args[0].clone()));
        }
        let pre = state.copy_ref();
        let mut outs = Vec::new();
        let mut states: Vec<State> = Vec::new();
        let n = groups.len();
        for (f, part) in groups {
            let mut st = if n == 1 { std::mem::replace(state, State::bottom()) } else { pre.copy_ref() };
            let mut a = args.clone();
            a[0] = part;
            if f == foid {
                self.sd_bypass = Some(foid);
            }
            outs.push(self.call_user(f, self_v.clone(), a, kwargs.clone(), vec![], star_kwargs.clone(), &mut st, node, None, entry));
            self.sd_bypass = None;
            states.push(st);
        }
        let mut it = states.into_iter();
        let mut out = it.next().unwrap_or_else(State::bottom);
        for mut s2 in it {
            out = out.join(&mut s2);
        }
        state.replace_with(out);
        join_values(outs)
    }
}

impl crate::interp::Analyzer {
    /// A call of a `functools.lru_cache` / `cache` function: with known arguments, the first
    /// call's result is the result of every later call with the same arguments.
    pub(crate) fn memo_call(&mut self, oid: ObjId, args: &[Value], kwargs: &IndexMap<StrId, Value>, state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        // a memoised function with known arguments: the first call's result is the
        // result of every later call with the same arguments
        let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
        let pos: Option<Vec<KeyId>> = args.iter().map(lit).collect();
        let kws: Option<Vec<(StrId, KeyId)>> = kwargs.iter().map(|(k, v)| lit(v).map(|x| (*k, x))).collect();
        // the cache is a hidden dict in the (flow-sensitive) state: on paths where the
        // first call happened the result is reused, elsewhere the call runs
        let key = pos.zip(kws).map(|(p, k)| {
            let mut parts = p;
            for (n, v) in k {
                parts.push(intern::key_tuple(vec![key_str_lit(&intern::str_get(n)), v]));
            }
            intern::key_tuple(parts)
        });
        let Some(mk) = key else {
            return self.call_user(oid, None, args.to_vec(), kwargs.clone(), vec![], vec![], state, node, call_ast, false);
        };
        let memo = self.alloc_raw(Kind::Dict, &format!("memo:{}", oid), "", true, "lru_cache", Payload::None);
        let row = state.row(memo, Ns::Item);
        // the hit / miss counts (cache_info), known while every call's outcome is
        let bump = |state: &mut State, name: &str, by_known: bool| {
            let k = key_str_lit(name);
            let cur = state.slot_opt(memo, Ns::Attr, k).unwrap_or_else(|| Value::lit(intern::key_int(0)));
            let nv = match (cur.kv.as_lit().and_then(intern::key_as_int), by_known) {
                (Some(n), true) => Value::lit(intern::key_int(n + 1)),
                _ => Value::data("count"),
            };
            state.set_slot(memo, Ns::Attr, k, nv, true);
        };
        if row.definite.contains(mk) {
            bump(state, "hits", true);
            return state.slot(memo, Ns::Item, mk);
        }
        if row.present.contains(mk) {
            bump(state, "hits", false);
            bump(state, "misses", false);
        } else {
            bump(state, "misses", true);
        }
        let r = self.call_user(oid, None, args.to_vec(), kwargs.clone(), vec![], vec![], state, node, call_ast, false);
        let out = if row.present.contains(mk) { r.join(&state.slot(memo, Ns::Item, mk)) } else { r };
        let row = state.row(memo, Ns::Item).store(mk, true);
        state.set_row(memo, Ns::Item, row);
        state.set_slot(memo, Ns::Item, mk, out.clone(), true);
        out
    }
}
