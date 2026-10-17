//! What every access shares: whether a handler guards it, the requirements a read records on
//! the inputs that provide a key, and running an access once per receiver. The accesses
//! themselves: `items.rs` (keys, positions, elements), `attrs.rs` (attributes), `keys.rs`.

use crate::domain::{join_values, Classification, Ns, ObjId, ReqInfo, Row, Value};
use crate::intern::{self, KeyId};
use crate::state::State;
use crate::source::SourceId;

use super::Analyzer;

/// The attributes every instance has (`object`'s, and its class's `__module__` / `__doc__`;
/// `__dict__` / `__weakref__` unless `__slots__` drop them: taken as there).
pub const OBJECT_ATTRS: &[&str] = &[
    "__class__", "__delattr__", "__dir__", "__doc__", "__eq__", "__format__", "__ge__", "__getattribute__", "__getstate__", "__gt__", "__hash__", "__init__", "__init_subclass__", "__le__", "__lt__", "__ne__", "__new__", "__reduce__", "__reduce_ex__", "__repr__", "__setattr__", "__sizeof__", "__str__", "__subclasshook__", "__module__", "__dict__", "__weakref__",
];

/// The exception names a handler must catch to guard a read that fails with a `KeyError` /
/// `IndexError` (an item), a `ValueError` or a pydantic `ValidationError` (a model's item
/// read, `Unwind::item_exc`), or an `AttributeError` (an attribute); `*` is a bare `except:`.
pub const VALUE_GUARDS: &[&str] = &["ValueError", "Exception", "BaseException", "*"];
pub const VALIDATION_GUARDS: &[&str] = &["ValidationError", "ValueError", "Exception", "BaseException", "*"];
pub const ITEM_GUARDS: &[&str] = &["KeyError", "LookupError", "Exception", "BaseException", "IndexError", "*"];
pub const ATTR_GUARDS: &[&str] = &["AttributeError", "Exception", "BaseException", "*"];

impl Analyzer {
    /// The handler names that guard a failing read in namespace `ns` here.
    fn guard_names(&self, ns: Ns) -> &'static [&'static str] {
        if ns != Ns::Item {
            return ATTR_GUARDS;
        }
        match self.unwind.item_exc {
            Some("ValueError") => VALUE_GUARDS,
            Some("ValidationError") => VALIDATION_GUARDS,
            _ => ITEM_GUARDS,
        }
    }

    /// The innermost handler that catches this namespace's error re-raises it.
    pub(in crate::interp) fn guard_reraises(&self, ns: Ns) -> bool {
        let want = self.guard_names(ns);
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
                        let n = crate::syntax::dotted_last(e);
                        super::exprs::is_exception_name(&n) && !user.contains(&n)
                    })).unwrap_or(false)
                })
                .unwrap_or(false)
        })
    }

    /// Does a running handler catch a failing read in namespace `ns` here?
    pub fn is_guarded(&self, ns: Ns) -> bool {
        let want = self.guard_names(ns);
        self.guards().any(|g| g.iter().any(|x| want.contains(&x.as_str())))
    }

    /// The records among the objects `v` may be.
    pub fn record_objects(&self, v: &Value) -> Vec<u32> {
        v.pts.iter().filter(|o| self.heap.is_record(*o)).collect()
    }

    /// `require_providers_k` for an item read of no particular key.
    pub fn require_providers(&mut self, cls: &Classification, row: &Row, with_alts: bool, state: &mut State) {
        self.require_providers_k(cls, row, with_alts, state, None, Ns::Item)
    }

    /// The access of `key` needs it of the inputs that may provide it; a key the program
    /// stored on some path is needed only on the others (sometimes).
    pub fn require_providers_k(&mut self, cls: &Classification, row: &Row, with_alts: bool, state: &mut State, key: Option<KeyId>, ns: Ns) {
        let srcs: Vec<SourceId> = cls.providers.iter().map(|(s, _)| *s).collect();
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
                        state.reqs.set(src, t, ReqInfo { attr: true, ..cur });
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

    /// A receiver of several objects (or objects and a scalar) some of which run user code for
    /// the operation (`dispatch`): those are alternatives, one state each (the second must not
    /// see the first's effects, nor the first's raise end the others' paths); the other objects
    /// are one more (no code of theirs runs), the scalar another. None: no object runs code
    /// (or there is only one part), the operation runs on `obj` as it is.
    pub(crate) fn per_receiver(&mut self, obj: &Value, dispatch: &dyn Fn(&Analyzer, u32) -> bool, state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &Value, &mut State) -> Value) -> Option<Value> {
        if obj.pts.len() + usize::from(!obj.kv.is_bot()) <= 1 {
            return None;
        }
        let (runs, rest): (Vec<ObjId>, Vec<ObjId>) = obj.pts.iter().partition(|o| dispatch(self, *o));
        if runs.is_empty() {
            return None;
        }
        let mut parts: Vec<Value> = runs.into_iter().map(Value::obj).collect();
        if !rest.is_empty() {
            parts.push(Value::objs(crate::domain::ObjSet::from_vec(rest)));
        }
        if !obj.kv.is_bot() {
            parts.push(Value::kv_only(obj.kv.clone()));
        }
        let mut outs: Vec<Value> = Vec::new();
        let mut acc: Option<State> = None;
        let saved = std::mem::replace(&mut self.mode.one_of_several, true);
        for p in parts {
            let mut st = state.copy();
            let v = f(self, &p, &mut st);
            // (a part whose access raised gives nothing)
            if st.reachable {
                outs.push(v);
            }
            acc = Some(match acc {
                None => st,
                Some(mut a) => a.join(&mut st),
            });
        }
        self.mode.one_of_several = saved;
        if let Some(a) = acc {
            state.replace_with(a);
        }
        Some(join_values(outs))
    }
}
