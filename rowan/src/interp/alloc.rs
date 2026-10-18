//! Allocation policy: which object an allocation at a site gives in the running context —
//! a strong object, a strong *recent* one inside a loop (folded into the site's summary at the
//! back-edge, `rename_recent`), a variant while the site's earlier object is still live, or the
//! merged summary (an instance a call makes there: the call's recent one, `call_recent`). The
//! objects themselves are the `Heap` (`interp/heap.rs`).

use rustc_hash::FxHashSet;

use super::{site_string, Analyzer, Payload};
use crate::domain::{Ns, ObjId, Row};
use crate::frontend::Node;
use crate::source::{source_label, Source, SourceKind};
use crate::state::{mark_merged, Kind, State};

impl Analyzer {
    // ------------------------------------------------------------ allocation
    /// The one object of a site in no context (a builtin's, a model's shared object).
    pub(in crate::interp) fn alloc_global(&mut self, kind: Kind, site: &str, label: &str, payload: Payload) -> ObjId {
        let label = if label.is_empty() { site } else { label };
        self.heap.alloc_raw(kind, site, "", true, label, payload)
    }

    /// Is the running code a call the main file's module code makes (no other module's code
    /// below it: not an import, not an entry run)?
    fn in_main_call(&self) -> bool {
        let fs = &self.cx.frames;
        fs.len() > 1 && fs[0].kind == super::FrameKind::Module && fs[0].module == self.main_module() && fs[0].ctx.is_empty() && fs[1..].iter().all(|f| f.kind == super::FrameKind::Function)
    }

    /// A node's site label (`site_string`).
    pub(in crate::interp) fn site_of(&self, node: &Node) -> String {
        site_string(node)
    }

    /// Allocate an object at `site` (string) in the current frame's context.  `single`:
    /// explicit strength; `None` = the frame decides (strong outside loops; inside a loop a
    /// strong *recent* object with the recency abstraction, else the weak per-site object).
    #[allow(clippy::too_many_arguments)]
    pub fn alloc(&mut self, kind: Kind, site: &str, state: &mut State, label: &str, payload: Payload, row: Option<Row>, ns: Ns, single: Option<bool>, ctx: Option<&str>) -> ObjId {
        let fr = self.frame();
        let ctx = ctx.map(|c| c.to_string()).unwrap_or_else(|| fr.ctx.clone());
        let label = if label.is_empty() { format!("{}@{}", kind.name(), site) } else { label.to_string() };
        // inside unrolled loop iterations each iteration is straight-line code: its own site
        let site_buf;
        let site: &str = if !fr.unroll.is_empty() {
            site_buf = format!("{}#{}", site, fr.unroll.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("."));
            &site_buf
        } else {
            site
        };
        let (single, recent) = match single {
            Some(x) => (x, false),
            None => {
                let strong = fr.single && fr.loop_depth == 0;
                if !strong && fr.single_nl && !self.cx.recent_stack.is_empty() {
                    (true, true)
                } else {
                    (strong, false)
                }
            }
        };
        let oid = if recent {
            self.alloc_recent(kind, site, &ctx, &label, payload)
        } else {
            let existed = self.heap.find(kind, site, &ctx);
            let payload2 = payload.clone();
            self.heap.from_main = self.in_main_call();
            let mut oid = self.heap.alloc_raw(kind, site, &ctx, single, &label, payload);
            self.heap.from_main = false;
            // (made earlier in the same model call: held by the model's own code)
            let model_held = self.cx.model_depth > 0 && self.cx.model_born.get(&oid) == Some(&self.cx.model_epoch);
            if self.cx.model_depth > 0 {
                self.cx.model_born.insert(oid, self.cx.model_epoch);
            }
            // (past the hard deadline no walk: the site's object is taken as still in use,
            // so it is merged — weak, always sound)
            // (an unknown call's result allocated again: taken as still in use without a walk,
            // and merged — its rows are open anyway; walking for it dominated large runs)
            // (an object already merged — its site made again while in use, past its variants —
            // stays merged without a walk: such walks found nothing in their budget)
            let hard = existed == Some(oid) && (self.budget.over_hard_deadline() || (kind == Kind::Unknown) || !self.heap.obj(oid).single);
            if existed == Some(oid) && (hard || model_held || (self.has_facts(oid, state) && !self.live_among(&[oid], state).is_empty())) {
                // allocated again while the earlier object is still reachable (a method run
                // twice on one receiver, a call repeated in a loop)
                if single && !self.cx.recent_stack.is_empty() && !hard {
                    // inside a loop: a strong recent object, folded into the summary object
                    // at the loop's back-edge (the earlier object becomes that summary)
                    oid = self.alloc_recent(kind, site, &ctx, &label, payload2);
                } else {
                    // another object of the site while the first is still in use: a variant
                    // of its own (a method run twice on one receiver keeps both records), up
                    // to a few; past them one summary object, weak (no longer one key)
                    let variant = if single && !hard { self.alloc_variant(kind, site, &ctx, single, &label, &payload2, state) } else { None };
                    match variant {
                        Some(v) => oid = v,
                        None => {
                            self.heap.obj_mut(oid).single = false;
                            mark_merged(oid);
                            if let Some(k) = self.heap.keys.identity_keys.borrow_mut().remove(&oid) {
                                self.heap.keys.identity_objs.borrow_mut().remove(&k);
                            }
                        }
                    }
                }
            }
            oid
        };
        let oid = if single && !recent { self.call_recent(kind, oid, &label, state).unwrap_or(oid) } else { oid };
        self.born_facts(oid, kind, state, row, ns);
        oid
    }

    /// Recency per call: an instance a call makes where its site's object is a summary (merged,
    /// or past the site's `MAX_CLONES`) is a strong *recent* object of the call's own, folded
    /// into the summary when the call returns (`fold_call_recent`): what the call stores is
    /// there for sure until then, and joins the summary with every key it got. Not while the
    /// recent object is in use: made by a running call or held on a path, or an identity key. Instances of the program's classes only (no
    /// side tables of models hold them).
    fn call_recent(&mut self, kind: Kind, oid: ObjId, label: &str, state: &State) -> Option<ObjId> {
        if kind != Kind::Instance || self.heap.obj(oid).single || self.budget.over_hard_deadline() {
            return None;
        }
        let fr = self.frame();
        if fr.kind != super::FrameKind::Function || fr.pre_call.is_none() || fr.loop_depth > 0 {
            return None;
        }
        let (site, ctx, payload) = {
            let o = self.heap.obj(oid);
            (o.site.to_string(), o.ctx.to_string(), o.payload.clone())
        };
        let Payload::Cls(c) = payload else { return None };
        if self.heap.class_info_opt(c).is_none_or(|ci| ci.open_base) {
            return None;
        }
        // (one per call depth: a recursive call's own)
        let rsite = format!("{}#call", site);
        let rctx = format!("{}#{}", ctx, self.cx.frames.len());
        if let Some(r) = self.heap.find(kind, &rsite, &rctx) {
            let in_use = self.has_facts(r, state) || self.cx.frames.iter().any(|f| f.recent.iter().any(|x| x.0 == r)) || self.heap.keys.identity_keys.borrow().contains_key(&r);
            if in_use {
                return None;
            }
        }
        let r = self.heap.alloc_raw(kind, &rsite, &rctx, true, label, payload);
        if !self.heap.obj(r).single {
            return None;
        }
        self.frame_mut().recent.push((r, kind, site, ctx));
        Some(r)
    }

    /// A strong *recent* object of the site in the running loop iteration (folded into the
    /// site's summary object at the back-edge, `rename_recent`).
    fn alloc_recent(&mut self, kind: Kind, site: &str, ctx: &str, label: &str, payload: Payload) -> ObjId {
        let rsite = format!("{}#recent", site);
        let oid = self.heap.alloc_raw(kind, &rsite, ctx, true, label, payload);
        let top = self.cx.recent_stack.last_mut().unwrap();
        if !top.iter().any(|(o, _, _, _)| *o == oid) {
            top.push((oid, kind, site.to_string(), ctx.to_string()));
        }
        oid
    }

    /// Another object of the site while the first is still in use: the first of its variants
    /// (up to 8) nothing reaches any more (their liveness in one walk); None: all in use.
    #[allow(clippy::too_many_arguments)]
    fn alloc_variant(&mut self, kind: Kind, site: &str, ctx: &str, single: bool, label: &str, payload2: &Payload, state: &State) -> Option<ObjId> {
        // (the variants made so far, their liveness in one walk)
        let made: Vec<(String, Option<ObjId>)> = (1..=8usize).map(|k| {
            let vctx = format!("{}~{}", ctx, k);
            let v = self.heap.find(kind, site, &vctx);
            (vctx, v)
        }).collect();
        let first_free = made.iter().position(|(_, v)| v.is_none()).unwrap_or(made.len());
        let olds: Vec<ObjId> = made[..first_free].iter().filter_map(|(_, v)| *v).collect();
        let olds: Vec<ObjId> = olds.into_iter().filter(|o| self.has_facts(*o, state)).collect();
        let live = if olds.is_empty() { FxHashSet::default() } else { self.live_among(&olds, state) };
        let pick = made.iter().position(|(_, v)| match v { None => true, Some(o) => !live.contains(o) });
        if let Some(i) = pick {
            return Some(self.heap.alloc_raw(kind, site, &made[i].0, single, label, payload2.clone()));
        }
        None
    }

    /// An object made here: its length unknown until the code making it says; a record's row
    /// (joined into the old one for a weak object; a strong one starts over).
    fn born_facts(&mut self, oid: ObjId, kind: Kind, state: &mut State, row: Option<Row>, ns: Ns) {
        let obj_single = self.heap.obj(oid).single;
        if obj_single {
            self.heap.renew(oid);
        }
        // born here: its length is unknown until the code making it says
        state.set_lens(oid, (None, 0, 0));
        if kind.is_record() {
            let fresh = row.unwrap_or_default();
            if state.has_row(oid, ns) && !obj_single {
                let j = state.row(oid, ns).join(&fresh);
                state.set_row(oid, ns, j);
            } else {
                state.set_row(oid, ns, fresh);
                if obj_single {
                    // strong re-allocation: drop stale slots of this object
                    state.drop_obj(oid);
                }
            }
        } else if kind.is_container() && obj_single {
            state.drop_obj(oid);
            state.elems.remove(&oid);
        }
    }

    /// `alloc` at an AST node.
    /// A model's `ctx` is a suffix of the frame's context: the same code run from two call
    /// contexts makes two objects.
    pub fn alloc_at(&mut self, kind: Kind, node: &Node, state: &mut State, label: &str, payload: Payload, row: Option<Row>, ns: Ns, single: Option<bool>, ctx: Option<&str>) -> ObjId {
        let site = site_string(node);
        let joined: Option<String> = match (&self.cx.alloc_extra, ctx) {
            (Some(x), Some(c)) => Some(format!("{}/{}", x, c)),
            (Some(x), None) => Some(x.clone()),
            (None, c) => c.map(|c| c.to_string()),
        };
        let ctx = joined.as_deref();
        let full = ctx.map(|c| format!("{}/{}", self.frame().ctx, c));
        self.alloc(kind, &site, state, label, payload, row, ns, single, full.as_deref())
    }

    /// Short form used by most models: a record-ish object at a node with a row.
    pub fn alloc_simple(&mut self, kind: Kind, node: &Node, state: &mut State, label: &str) -> ObjId {
        self.alloc_at(kind, node, state, label, Payload::None, None, Ns::Item, None, None)
    }

    /// The unknown object standing for input `site` (of `kind`): open through it, labelled
    /// `label` (its source's label if empty); a parameter or an unbound name also gets its name
    /// as the report's symbol.
    pub fn unknown_source(&mut self, site: &str, kind: SourceKind, state: &mut State, label: &str) -> ObjId {
        self.unknown_source_in(site, kind, state, label, "")
    }

    /// `unknown_source`'s object in context `ctx`: another object open through the same input.
    pub(in crate::interp) fn unknown_source_in(&mut self, site: &str, kind: SourceKind, state: &mut State, label: &str, ctx: &str) -> ObjId {
        let src = Source::new(site, kind);
        let slabel = source_label(src);
        let label = if label.is_empty() { slabel.clone() } else { label.to_string() };
        let oid = self.heap.alloc_raw(Kind::Unknown, &format!("{}:{}", kind.name(), slabel), ctx, true, &label, Payload::None);
        for n in [Ns::Item, Ns::Attr] {
            if !state.has_row(oid, n) {
                state.set_row(oid, n, Row::with_source(src));
            }
        }
        if !self.names.has(oid) {
            let sym = match kind {
                SourceKind::Param => Some(label.trim_start_matches("param ").to_string()),
                SourceKind::Name => Some(label.trim_start_matches('?').to_string()),
                _ => None,
            };
            if let Some(sym) = sym.filter(|x| !x.is_empty()) {
                let scope = if kind == SourceKind::Param { self.entry.scope.unwrap_or_else(|| self.frame().func.unwrap_or(0)) } else { 0 };
                self.names.set(oid, sym, scope);
            }
        }
        oid
    }
}
