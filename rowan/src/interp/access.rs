//! What every access shares: whether a handler guards it, the requirements a read records on
//! the inputs that provide a key, and running an access once per receiver. The accesses
//! themselves: `items.rs` (keys, positions, elements), `attrs.rs` (attributes), `keys.rs`.

use ruff_python_ast as ast;

use crate::domain::{Classification, Ns, Row, Value};
use crate::intern::{self, KeyId};
use crate::state::State;

use super::{Analyzer, ATTR_GUARDS, ITEM_GUARDS};

/// The attributes every instance has (`object`'s, and its class's `__module__` / `__doc__`;
/// `__dict__` / `__weakref__` unless `__slots__` drop them: taken as there).
pub const OBJECT_ATTRS: &[&str] = &[
    "__class__", "__delattr__", "__dir__", "__doc__", "__eq__", "__format__", "__ge__", "__getattribute__", "__getstate__", "__gt__", "__hash__", "__init__", "__init_subclass__", "__le__", "__lt__", "__ne__", "__new__", "__reduce__", "__reduce_ex__", "__repr__", "__setattr__", "__sizeof__", "__str__", "__subclasshook__", "__module__", "__dict__", "__weakref__",
];

impl Analyzer {
    /// The innermost handler that catches this namespace's error re-raises it.
    pub(in crate::interp) fn guard_reraises(&self, ns: Ns) -> bool {
        let want: &[&str] = if ns == Ns::Item { match self.unwind.item_exc { Some("ValueError") => super::VALUE_GUARDS, Some("ValidationError") => super::VALIDATION_GUARDS, _ => ITEM_GUARDS } } else { ATTR_GUARDS };
        for g in self.guards().rev() {
            if g.iter().any(|x| want.contains(&x.as_str())) {
                return g.contains("!reraise");
            }
        }
        false
    }

    /// Some class of `cls`'s MRO derives from a builtin exception class directly.
    pub(in crate::interp) fn builtin_exc_base(&self, cls: u32) -> bool {
        // (external bases are not in `bases`: read the class statement's base expressions)
        self.heap.mro(cls).iter().any(|c| {
            self.heap.class_info_opt(*c)
                .map(|ci| {
                    let user: Vec<String> = ci.bases.iter().filter_map(|b| self.heap.class_info_opt(*b).map(|x| x.name.clone())).collect();
                    ci.node.arguments.as_ref().map(|a| a.args.iter().any(|e| {
                        let n = super::dotted_last(e);
                        super::exprs::is_exception_name(&n) && !user.contains(&n)
                    })).unwrap_or(false)
                })
                .unwrap_or(false)
        })
    }

    pub fn is_guarded(&self, ns: Ns) -> bool {
        let want: &[&str] = if ns == Ns::Item { match self.unwind.item_exc { Some("ValueError") => super::VALUE_GUARDS, Some("ValidationError") => super::VALIDATION_GUARDS, _ => ITEM_GUARDS } } else { ATTR_GUARDS };
        self.guards().any(|g| g.iter().any(|x| want.contains(&x.as_str())))
    }

    pub fn record_objects(&self, v: &Value) -> Vec<u32> {
        v.pts.iter().filter(|o| self.heap.is_record_like(*o)).collect()
    }

    pub fn require_providers(&mut self, cls: &Classification, row: &Row, with_alts: bool, state: &mut State) {
        self.require_providers_k(cls, row, with_alts, state, None, Ns::Item)
    }

    /// The access of `key` needs it of the inputs that may provide it; a key the program
    /// stored on some path is needed only on the others (sometimes).
    pub fn require_providers_k(&mut self, cls: &Classification, row: &Row, with_alts: bool, state: &mut State, key: Option<KeyId>, ns: Ns) {
        let srcs: Vec<crate::domain::SourceId> = cls.providers.iter().map(|(s, _)| *s).collect();
        let sometimes = key.map(|k| row.stored.contains(k)).unwrap_or(false);
        for (src, terms) in &cls.providers {
            for t in terms.iter() {
                // (an access needed on some paths only adds nothing to whether it is always)
                let was_always = state.reqs.get(src, &t).map(|r| r.always).unwrap_or(false);
                if with_alts {
                    let alts = intern::IdSet::from_iter(srcs.iter().copied().filter(|x| x != src));
                    state.require(*src, t, alts, row.mutators.clone(), terms.clone());
                } else {
                    state.require(*src, t, intern::IdSet::new(), row.mutators.clone(), intern::IdSet::new());
                }
                if ns == Ns::Attr {
                    if let Some(cur) = state.reqs.get(src, &t).filter(|r| !r.attr) {
                        state.reqs.set(src, t, crate::domain::ReqInfo { attr: true, ..cur });
                    }
                }
                if sometimes && !was_always {
                    if let Some(cur) = state.reqs.get(src, &t) {
                        state.reqs.set(src, t, cur.sometimes());
                    }
                }
            }
        }
    }


    /// A receiver of several objects (or objects and a scalar) one of which runs user code for
    /// the operation: they are alternatives, one state each (the second must not see the
    /// first's effects, nor the first's raise end the others' paths).
    pub(crate) fn split_dispatch(&self, obj: &Value, dispatch: &dyn Fn(&Analyzer, u32) -> bool) -> bool {
        obj.pts.len() + usize::from(!obj.kv.is_bot()) > 1 && obj.pts.iter().any(|o| dispatch(self, o))
    }

    pub(crate) fn per_receiver(&mut self, obj: &Value, state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &Value, &mut State) -> Value) -> Value {
        let mut parts: Vec<Value> = obj.pts.iter().map(Value::obj).collect();
        if !obj.kv.is_bot() {
            parts.push(Value::kv_only(obj.kv.clone()));
        }
        let mut outs: Vec<Value> = Vec::new();
        let mut acc: Option<State> = None;
        for p in parts {
            let mut st = state.copy();
            outs.push(f(self, &p, &mut st));
            acc = Some(match acc {
                None => st,
                Some(mut a) => a.join(&mut st),
            });
        }
        if let Some(a) = acc {
            state.replace_with(a);
        }
        crate::domain::join_values(outs)
    }



}

/// All statements nested in a body (ast.walk restricted to statements).
/// builtin container classes a field may be declared with (they select a model family)

pub fn walk_stmts(body: &'static [ast::Stmt]) -> Vec<&'static ast::Stmt> {
    let mut out: Vec<&'static ast::Stmt> = Vec::new();
    fn go(s: &'static ast::Stmt, out: &mut Vec<&'static ast::Stmt>) {
        out.push(s);
        match s {
            ast::Stmt::FunctionDef(f) => f.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::ClassDef(c) => c.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::For(f) => {
                f.body.iter().for_each(|x| go(x, out));
                f.orelse.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::While(w) => {
                w.body.iter().for_each(|x| go(x, out));
                w.orelse.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::If(i) => {
                i.body.iter().for_each(|x| go(x, out));
                i.elif_else_clauses.iter().for_each(|c| c.body.iter().for_each(|x| go(x, out)));
            }
            ast::Stmt::With(w) => w.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::Try(t) => {
                t.body.iter().for_each(|x| go(x, out));
                for h in &t.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    h.body.iter().for_each(|x| go(x, out));
                }
                t.orelse.iter().for_each(|x| go(x, out));
                t.finalbody.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::Match(m) => m.cases.iter().for_each(|c| c.body.iter().for_each(|x| go(x, out))),
            _ => {}
        }
    }
    body.iter().for_each(|x| go(x, &mut out));
    out
}
