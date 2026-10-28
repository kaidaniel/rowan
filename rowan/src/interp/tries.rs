//! Exceptions and the other ways out of running code. Every point of a running `try` body
//! (and of the calls it makes) that may raise records the state there (`record_raise_state`),
//! joined per exception class; handlers start from the raises they may catch, the rest go on
//! through the `finally` to the enclosing try. A `with` is a try whose finally is `__exit__`.
//! `break` / `continue` run the finally blocks of their loop first.
//!
//! For a call memo: `Unwind` is the dynamic context of the running code (restored by the code
//! that pushes it), except what a run records into the tries of its callers: a hit would have to
//! replay the raise states its run recorded there.

use ruff_python_ast as ast;
use rustc_hash::FxHashSet;

use super::{Analyzer, Payload};
use super::stmts::{add_facts, name_facts};
use crate::domain::{join_values, Ns, ObjId, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s};
use crate::state::{mark_merged, Joins, Kind, State};
use crate::syntax::{collect_assigned, contains_jump, contains_return, dotted_last};

/// An exception class's name and its bases' names; `true`: that is the whole MRO.
pub type ExcNames = (std::rc::Rc<Vec<String>>, bool);
/// A raise a try recorded: its class (None: not known), the state there, the exception object.
pub type RaiseRec = (Option<ExcNames>, State, Value);

/// The states of a try's recorded raises, joined (None: no raise recorded).
pub fn join_raise_recs(recs: &[RaiseRec]) -> Option<State> {
    State::join_all(recs.iter().map(|r| r.1.copy()).collect())
}

/// The class of an exception that code rowan cannot see (or a model that does not say which)
/// may raise: any class, which no handler surely catches or surely misses.
pub fn any_exc() -> ExcNames {
    (std::rc::Rc::new(Vec::new()), false)
}

/// Is this the class `any_exc` stands for?
pub fn is_any_exc(c: &Option<ExcNames>) -> bool {
    c.as_ref().is_some_and(|(n, full)| n.is_empty() && !full)
}

/// A running try context. Every point of its body (and of the calls it makes) that may raise
/// records the state there, with the try's frame's names and the names possibly unbound: a
/// handler starts from the join of the states whose exceptions it may catch (its `as` name
/// bound to those of its classes), a finally on an exceptional way out from those it does not.
///
/// What may raise: a `raise`; a read that may fail (also where no handler catches it and only a
/// `finally` or `__exit__` that runs code sees it, `finally_sees`); an operator (types that may
/// not fit, a divisor that may be 0, a tensor dim's too: `scalar_binop_raises`; an ordering,
/// `ordering_raises`; `%` formatting, `percent_raises`; an unknown object's operator is unknown
/// code); an attribute a literal's type lacks (`literals_lack_attr`; an untyped unknown's may
/// lack any), a str's or dict's method of an unknown; a subscript or `del` of an unknown scalar,
/// which may be any object (`unknown_scalar_may_raise`: anything); a model's raise; any unknown
/// call (class `any_exc`).
pub struct TryCtx {
    /// the exception class names its handlers catch (`*`: a bare except; `!reraise`: every
    /// handler ends by re-raising)
    pub caught: FxHashSet<String>,
    /// what it catches by name, its catch-alls aside (`Exception`, a bare `except:`): a guard
    /// of the reads in the functions it calls too (`Analyzer::is_guarded`)
    pub named: FxHashSet<String>,
    /// the frame depth and caller-env depth it runs in: a raise inside a callee hands the
    /// handler the try frame's locals
    pub frames: usize,
    pub envs: usize,
    /// the state before it and the allocation serial then (what was made since, and nothing
    /// reaches after the unwind, is garbage in a raise state)
    pub pre: State,
    pub mark: u64,
    /// the raises recorded, per class: (class, the states, the exceptions, how many items the
    /// generator `gen` had given then, and for the raises that leave the program — one entry
    /// for all their classes — the other classes), joined when it ends
    pub recs: Vec<(Option<ExcNames>, Joins, Value, Option<usize>, Option<Vec<Option<ExcNames>>>)>,
    /// a generator run whole for a loop: its raises are taken where the loop asks for the item
    /// it fails to give (`genraise.rs`), kept apart by how many it gave before
    pub gen: Option<ObjId>,
    /// the last raise recorded, before collection (a loop of calls raising at one point adds
    /// nothing)
    pub last: Option<(State, Option<ExcNames>, Value)>,
    /// the classes its handlers catch (`handler_classes`; one may-catch-anything of `caught`
    /// for a try a model runs) and whether a raise leaving it runs code on its way out (a
    /// finally block, managers' `__exit__`): what of a raise it can see (`sees`)
    pub hclasses: Vec<(FxHashSet<String>, bool)>,
    pub runs: bool,
}

impl TryCtx {
    /// Can a raise of class `cls` change what this try's handlers or way out see? Not when
    /// every handler surely misses it and nothing runs on the way out: the raise only passes
    /// through, to the tries around it.
    pub(in crate::interp) fn sees(&self, cls: &Option<ExcNames>) -> bool {
        self.runs || self.hclasses.iter().any(|hc| handler_matches(hc, cls) != Some(false))
    }
}

/// What a try context recorded while its body ran (`Analyzer::leave_try`).
pub struct TryExit {
    /// the states and exceptions at the recorded raises, joined per class
    pub recs: Vec<RaiseRec>,
    /// per raise, how many items its generator had given (`TryCtx::gen`; None: not known)
    pub pos: Vec<Option<usize>>,
}

impl TryExit {
    /// The join of the states at the recorded raises (None: none recorded).
    pub fn raised(&self) -> Option<State> {
        join_raise_recs(&self.recs)
    }

    /// The exceptions raised at the recorded points.
    pub fn value(&self) -> Value {
        join_values(self.recs.iter().map(|r| r.2.clone()).collect::<Vec<_>>())
    }

    /// Code that may raise anything ran (a raise of `any_exc`).
    pub fn any(&self) -> bool {
        self.recs.iter().any(|r| is_any_exc(&r.0))
    }
}

/// A running loop: the states at its `break`s and `continue`s so far, joined.
#[derive(Default)]
pub struct LoopCtx {
    pub breaks: Option<State>,
    pub continues: Option<State>,
}

/// An `except` clause: the classes it names (None: a bare `except`), the name it binds, its
/// body (a `with` that swallows, `suppress(E)`: one that does nothing).
pub struct Handler {
    pub types: Option<Vec<&'static ast::Expr>>,
    pub name: Option<&'static str>,
    pub body: &'static [ast::Stmt],
    pub line: u32,
    /// it may catch any exception, or not (an `__exit__` that may return a true value)
    pub maybe: bool,
}

impl Handler {
    fn names(&self) -> FxHashSet<String> {
        match &self.types {
            None => ["*".to_string()].into_iter().collect(),
            Some(es) => es.iter().map(|e| dotted_last(e)).collect(),
        }
    }
}

/// What runs on the way out of a try statement: its finally block, or for a with statement its
/// managers' `__exit__`.
pub enum Finally {
    Block(&'static [ast::Stmt]),
    /// the managers, and the name of their exit method (`__exit__`, `__aexit__`)
    Exit(Vec<(Value, Node)>, &'static str),
}

impl Finally {
    pub(in crate::interp) fn is_empty(&self) -> bool {
        match self {
            Finally::Block(b) => b.is_empty(),
            Finally::Exit(m, _) => m.is_empty(),
        }
    }

    /// A `return` / `break` / `continue` in it ends an exception passing through.
    pub(in crate::interp) fn swallows(&self) -> bool {
        match self {
            Finally::Block(b) => contains_return(b) || contains_jump(b),
            Finally::Exit(..) => false,
        }
    }
}

/// The exceptions in flight and the running try statements, handlers, loops and finally
/// blocks.
#[derive(Default)]
pub struct Unwind {
    /// the running try contexts (innermost last): `try` statements, swallowing `with`s and the
    /// models' own catching frames
    pub tries: Vec<TryCtx>,
    /// the exception the next `record_raise_state` is about
    pub cur_exc: Option<Value>,
    /// the class of the exception being recorded (set beside `cur_exc`)
    pub cur_exc_cls: Option<ExcNames>,
    /// exception names handled by the running `except` clauses (innermost last): what a bare
    /// `raise` re-raises
    pub handling: Vec<FxHashSet<String>>,
    /// the exception values the running handlers hold (a bare `raise` re-raises them)
    pub handling_values: Vec<Value>,
    /// the running handlers that may catch GeneratorExit raised at a yield to close a generator
    /// run whole (`genclose.rs`): their frame depth, the yield's position (None: not known),
    /// and their place in `handling`
    pub closing: Vec<(usize, Option<usize>, usize)>,
    /// inside a call whose exceptions a thread / future keeps: reads still require their keys
    pub catching_depth: u32,
    /// the finally blocks of the running `try` statements: (`loop_stack` depth at the try,
    /// the block, not running yet): a `break` / `continue` runs those of its loop first
    pub finally_stack: Vec<(usize, std::rc::Rc<Finally>, bool)>,
    pub loop_stack: Vec<LoopCtx>,
    /// what the running `for` loops watch of what they iterate (innermost last)
    pub for_watch: Vec<super::loops::ForWatch>,
    /// the call being evaluated: where an exception a model raises is allocated
    pub exc_node: Option<Node>,
    pub item_exc: Option<&'static str>,
    /// what a missing key raises in the reads a model makes (pydantic's field reads: a
    /// ValidationError)
    pub item_exc_force: Option<&'static str>,
    /// a load guarded by a `try` handler is running (its key is still required)
    pub caught_load: bool,
}

impl Analyzer {
    /// A try context begins: raises inside it record the state there for its handlers and
    /// its finally; `pre` is the state before the body.
    /// `caught`: the classes its handlers name (a class's subclasses are caught too).
    pub fn enter_try(&mut self, caught: FxHashSet<String>, pre: &mut State) {
        let (frames, envs, mark) = (self.cx.frames.len(), self.cx.env_stack.len(), self.heap.serial());
        let named = crate::models::exceptions::expand_caught(caught.iter().filter(|n| !super::access::CATCH_ALL.contains(&n.as_str())).cloned().collect());
        let caught = crate::models::exceptions::expand_caught(caught);
        let hclasses = if caught.is_empty() { Vec::new() } else { vec![(caught.clone(), false)] };
        self.unwind.tries.push(TryCtx { caught, named, frames, envs, pre: pre.copy(), mark, recs: Vec::new(), last: None, gen: None, hclasses, runs: false });
    }

    /// The try context ends: what it recorded. What callees made since the try began and
    /// nothing reaches after the unwind (the try's names, the frames below it, the exception)
    /// is garbage in a raise state: a try around a large recursion would otherwise join every
    /// callee's objects into its handlers.
    pub fn leave_try(&mut self) -> TryExit {
        // (a generator's raises kept for its consumer's asking, `genraise.rs`: the rest here)
        if !self.gens.raises.is_empty() {
            self.flush_gen_raises(self.unwind.tries.len());
        }
        let Some(t) = self.unwind.tries.pop() else { return TryExit { recs: Vec::new(), pos: Vec::new() } };
        let (mut recs, mut pos): (Vec<RaiseRec>, Vec<Option<usize>>) = (Vec::new(), Vec::new());
        let mut more: Vec<(usize, Vec<Option<ExcNames>>)> = Vec::new();
        for (c, sts, e, p, m) in t.recs {
            let Some(st) = sts.finish() else { continue };
            if let Some(m) = m {
                more.push((recs.len(), m));
            }
            recs.push((c, st, e));
            pos.push(p);
        }
        for (_, st, exc) in recs.iter_mut() {
            let mut roots: Vec<ObjId> = Vec::new();
            self.state_roots(st, t.envs, t.frames, &mut roots);
            for (o, v) in st.lasts.iter() {
                roots.push(*o);
                v.push_objs(&mut roots);
            }
            exc.push_objs(&mut roots);
            self.collect_since(t.mark, &t.pre, roots, st, Some(t.envs));
        }
        // (the raises leaving the program: one state for each of their classes)
        for (i, m) in more {
            for c in m {
                let (st, e) = (recs[i].1.copy(), recs[i].2.clone());
                recs.push((c, st, e));
                pos.push(pos[i]);
            }
        }
        TryExit { recs, pos }
    }

    /// Which try contexts are running: how many, and the innermost one's start (its allocation
    /// serial then).
    pub(in crate::interp) fn try_mark(&self) -> (usize, u64) {
        (self.unwind.tries.len(), self.unwind.tries.last().map_or(0, |t| t.mark))
    }

    /// The exception classes the running try contexts catch (innermost last).
    pub fn guards(&self) -> impl DoubleEndedIterator<Item = &FxHashSet<String>> {
        self.unwind.tries.iter().map(|t| &t.caught)
    }

    /// An exception raised here (`cur_exc`, of class `cur_exc_cls`) enters the innermost
    /// running try: its handlers and finally start from this state (with the try frame's
    /// locals when the raise is inside a callee). False: no try is running.
    pub(in crate::interp) fn record_raise_state(&mut self, state: &mut State) -> bool {
        let mut exc = self.unwind.cur_exc.take().unwrap_or_else(|| Value::data("exception"));
        let cls = self.unwind.cur_exc_cls.take().or_else(|| self.exc_value_names(&exc));
        // (GeneratorExit raised at a yield, leaving the body of the generator run whole there)
        if self.record_close(&cls, state) {
            return true;
        }
        // (a raise leaving a recursive activation: its recursive calls raise it next round)
        if !self.recursion.collecting.is_empty() {
            self.collect_rec_raise(&cls, &exc, state);
        }
        let Some(t) = self.unwind.tries.last() else { return false };
        // (no running try sees it: it leaves the program, through ways out that run nothing)
        if !self.unwind.tries.iter().any(|t| t.sees(&cls)) {
            return true;
        }
        let (fl, el) = (t.frames, t.envs);
        let mut here = state.copy();
        // the try is in a `@contextmanager` generator whose with body is running (also from a
        // callee of the body): the generator's names
        let in_cm = self.gens.cm_frames.iter().rev().find(|(d, _, el0, _)| *d == fl && el <= *el0).map(|(_, env, _, ub)| (env.clone(), ub.clone()));
        // (the raise leaves the code it is in: a callee of the try's frame, or the with body
        // of the generator the try is in, at any depth)
        let leaves = in_cm.is_some() || self.cx.frames.len() != fl;
        // (with their possibly-unbound names, not those of the code the raise leaves)
        if let Some((env, ub)) = in_cm {
            here.env = env;
            here.unbound = ub;
            here.narrowed = Default::default();
        } else if self.cx.frames.len() < fl {
            if let Some((_, env, _, ub)) = self.gens.cm_frames.iter().rev().find(|(d, _, _, _)| *d == fl) {
                here.env = env.clone();
                here.unbound = ub.clone();
                here.narrowed = Default::default();
            }
        } else if self.cx.frames.len() > fl {
            here.env = self.cx.env_stack.get(el).cloned().unwrap_or_else(|| t.pre.env.clone());
            here.unbound = self.cx.unbound_stack.get(el).cloned().unwrap_or_else(|| t.pre.unbound.clone());
            here.narrowed = t.pre.narrowed.clone();
            here.ret = t.pre.ret.clone();
            here.exits = t.pre.exits.clone();
        }
        // (the recent objects of the calls the raise leaves join their summaries here: they
        // are other calls' once these return, `call_recent`)
        if let Some(i) = self.cx.frames.iter().skip(fl).position(|f| !f.recent.is_empty()) {
            if let Some(super::PreCall(mark, pre)) = self.cx.frames[fl + i].pre_call.clone() {
                let recs: Vec<_> = self.cx.frames[fl + i..].iter().flat_map(|f| f.recent.iter().cloned()).collect();
                self.fold_recent_in(&mut here, &pre, mark, recs, &mut exc, true);
            }
        }
        let Some(t) = self.unwind.tries.last() else { return false };
        // (the same state, class and exception as the last raise recorded in this try — a loop
        // of calls raising at one point: nothing to add)
        if let Some((ls, lc, le)) = &t.last {
            if *lc == cls && *le == exc && ls.same(&here) {
                return true;
            }
        }
        self.unwind.tries.last_mut().unwrap().last = Some((here.copy(), cls.clone(), exc.clone()));
        // (an object whose constructor the raise leaves, made in the try, is garbage in this
        // raise state unless what the try's frame reaches holds it: it is not the object a
        // later raise point holds once built)
        if leaves {
            let mark = self.unwind.tries.last().unwrap().mark;
            let half: Vec<ObjId> = self.cx.fresh_init.iter().filter(|(o, b)| !b.held && !self.heap.born_before(**o, mark)).map(|(o, _)| *o).collect();
            for o in half {
                let mut roots: Vec<ObjId> = Vec::new();
                self.state_roots(&here, el, fl, &mut roots);
                for (x, v) in here.lasts.iter() {
                    roots.push(*x);
                    v.push_objs(&mut roots);
                }
                exc.push_objs(&mut roots);
                self.drop_unreached(o, roots, &mut here, el);
            }
        }
        // (joined per class: a handler takes every raise of a class or none of them; what
        // callees made is collected when the try ends)
        let pos = self.unwind.tries.last().unwrap().gen.and_then(|g| super::genraise::items_given(&self.heap, g, &here));
        // (those leaving the program, whatever their class, in one entry: `leaves_program`)
        let leaving = self.leaves_program(&cls, &[]);
        let recs = &mut self.unwind.tries.last_mut().unwrap().recs;
        match recs.iter_mut().find(|r| r.3 == pos && if leaving { r.4.is_some() } else { r.4.is_none() && r.0 == cls }) {
            Some(r) => {
                r.1.add(here);
                r.2 = r.2.join(&exc);
                if let Some(m) = r.4.as_mut().filter(|m| r.0 != cls && !m.contains(&cls)) {
                    m.push(cls);
                }
            }
            None => {
                let mut j = crate::state::Joins::default();
                j.add(here);
                recs.push((cls, j, exc, pos, leaving.then(Vec::new)));
            }
        }
        true
    }

    /// The class names of an exception value (one class; None: not known).
    pub(in crate::interp) fn exc_value_names(&self, v: &Value) -> Option<super::ExcNames> {
        if !v.kv.is_bot() || v.pts.is_empty() {
            return None;
        }
        let mut out: Option<super::ExcNames> = None;
        for o in v.pts.iter() {
            let names = if let Some(c) = self.heap.instance_class(o) {
                self.exc_class_names(c)?
            } else if let Some(n) = self.m.exc_objs.get(&o) {
                super::builtin_exc_names(n)
            } else if self.heap.kind(o) == Kind::External && self.heap.class_info_opt(o).is_none() {
                // (a raised class: `raise KeyError`)
                super::builtin_exc_names(self.heap.external_qual(o).rsplit('.').next().unwrap_or(""))
            } else {
                return None;
            };
            match &out {
                None => out = Some(names),
                Some(prev) if *prev == names => {}
                Some(_) => return None,
            }
        }
        out
    }

    /// The names of a user exception class's MRO, with the builtin bases' names.
    pub(in crate::interp) fn exc_class_names(&self, cls: u32) -> Option<super::ExcNames> {
        let mro = self.heap.mro(cls);
        let mut names: Vec<String> = Vec::new();
        let mut complete = true;
        for c in mro.iter() {
            let ci = self.heap.class_info_opt(*c)?;
            names.push(ci.name.clone());
            let user: Vec<String> = ci.bases.iter().filter_map(|b| self.heap.class_info_opt(*b).map(|x| x.name.clone())).collect();
            for e in ci.node.arguments.as_ref().map(|a| a.args.iter().collect::<Vec<_>>()).unwrap_or_default() {
                let n = crate::syntax::dotted_last(e);
                if user.contains(&n) || n == "object" {
                    continue;
                }
                let (bn, full) = super::builtin_exc_names(&n);
                complete &= full;
                for b in bn.iter() {
                    if !names.contains(b) {
                        names.push(b.clone());
                    }
                }
            }
        }
        Some((std::rc::Rc::new(names), complete))
    }

    /// An exception named `name` is raised here (the rest of the path is dead).
    pub fn raise_here(&mut self, name: &str, state: &mut State) {
        let cls = self.unwind.cur_exc_cls.clone().unwrap_or_else(|| super::builtin_exc_names(name));
        self.may_raise_here(name, state);
        self.note_items_at_raise(&Some(cls), state);
        state.reachable = false;
    }

    /// Does a handler of a running try begun at frame depth `depth` or deeper surely catch
    /// exceptions of class `cls`?
    pub(super) fn surely_caught_from(&self, depth: usize, cls: &Option<super::ExcNames>) -> bool {
        self.unwind.tries.iter().any(|t| t.frames >= depth && t.hclasses.iter().any(|hc| handler_matches(hc, cls) == Some(true)))
    }

    /// An exception named `name` may be raised here: a running try records this state for its
    /// handlers and finally; otherwise the enclosing call raises.
    pub fn may_raise_here(&mut self, name: &str, state: &mut State) {
        let cls = self.unwind.cur_exc_cls.take().unwrap_or_else(|| super::builtin_exc_names(name));
        // a builtin exception a model raises, for a handler that may catch it: an object of its
        // class, one per class (its args not known)
        // (some running try's guard catches exceptions of this class)
        let guarded = self.guards().any(|g| g.contains("*") || cls.0.iter().any(|n| g.contains(n)) || (!cls.1 && (g.contains("Exception") || g.contains("BaseException"))));
        if self.unwind.cur_exc.is_none() && guarded && self.unwind.exc_node.is_some() {
            let n = name.rsplit('.').next().unwrap_or(name);
            if super::builtin_exc_names(n).1 && super::is_builtin_name(n) {
                let o = self.heap.alloc_raw(Kind::Namespace, &format!("exc:{}", n), "", false, n, Payload::None);
                mark_merged(o);
                if !state.has_row(o, Ns::Attr) {
                    state.set_row(o, Ns::Attr, crate::domain::Row::with_source(crate::source::Source::new(n, crate::source::SourceKind::Data)));
                }
                state.set_slot(o, Ns::Attr, key_str_lit("args"), Value::data("args"), false);
                self.m.exc_objs.insert(o, n.to_string());
                self.unwind.cur_exc = Some(Value::obj(o));
            }
        }
        self.unwind.cur_exc_cls = Some(cls);
        self.record_raise_state(state);
        self.unwind.cur_exc_cls = None;
    }

    /// Code that may raise anything ran here (unknown code, a model that does not say what it
    /// raises): a running try records this state.
    pub fn flag_may_raise(&mut self, state: &mut State) {
        self.unwind.cur_exc = None;
        self.unwind.cur_exc_cls = Some(super::any_exc());
        self.record_raise_state(state);
        self.unwind.cur_exc_cls = None;
    }

    /// A raise a try context recorded that leaves it (no handler of it surely catches it): the
    /// enclosing try records it (with no try running, it leaves the program).
    pub fn reraise_rec(&mut self, rec: super::RaiseRec) {
        let (cls, mut st, exc) = rec;
        self.unwind.cur_exc = if exc.pts.is_empty() { None } else { Some(exc) };
        self.unwind.cur_exc_cls = cls;
        self.record_raise_state(&mut st);
        self.unwind.cur_exc = None;
        self.unwind.cur_exc_cls = None;
    }

    /// May the manager's class's `__exit__` return a true value (a `return` of anything but
    /// `None` / `False`)? Then it may swallow the body's exception.
    pub(super) fn exit_swallows(&mut self, v: &Value, exit: &str) -> bool {
        for oid in v.pts.iter() {
            if self.heap.kind(oid) != Kind::Instance {
                continue;
            }
            if self.own_methods(oid, exit).unwrap_or_default().into_iter().any(|f| crate::syntax::may_return_true(self.heap.func_info(f).node.body())) {
                return true;
            }
        }
        false
    }

    /// Does `fin` run code on an exceptional way out: a finally block, a manager's `__exit__`
    /// (not those of `redirect_stdout`, `nullcontext`, a file or a StringIO, which run none)?
    fn runs_code(&self, fin: &Finally) -> bool {
        const INERT: &[&str] = &["stdobj.redirect", "stdobj.nullcontext", "stdobj.stringio", "stdobj.file"];
        let inert = |v: &Value| v.kv.is_bot() && v.pts.iter().all(|o| self.heap.kind(o) == Kind::Namespace && matches!(&self.heap.obj(o).payload, Payload::Method(q, _) if INERT.contains(&q.as_str())));
        match fin {
            Finally::Block(b) => !b.is_empty(),
            Finally::Exit(managers, _) => !managers.iter().all(|(v, _)| inert(v)),
        }
    }

    /// A finally block run on the way out of its try (not again by a `break` / `continue`
    /// inside it); for a with statement, the managers' `__exit__` (innermost first), with an
    /// unknown exception on an exceptional way out.
    pub(in crate::interp) fn run_finally(&mut self, fin: &std::rc::Rc<Finally>, st: &mut State, exc: bool) {
        let idx: Vec<usize> = self.unwind.finally_stack.iter().enumerate().filter(|(_, (_, b, a))| *a && std::rc::Rc::ptr_eq(b, fin)).map(|(i, _)| i).collect();
        for i in &idx {
            self.unwind.finally_stack[*i].2 = false;
        }
        match &**fin {
            Finally::Block(body) => self.exec_block(body, st),
            Finally::Exit(managers, exit) => {
                for (v, n) in managers.iter().rev() {
                    if !st.reachable {
                        break;
                    }
                    let a = if exc { Value::data("exc") } else { Value::none() };
                    let r = self.call_method_with(v, exit, vec![a.clone(), a.clone(), a], n, st);
                    if let Some(r) = r.filter(|_| *exit == "__aexit__") {
                        self.await_value(&r, st);
                    }
                    // an ExitStack: its callbacks, last registered first; closing(x): x.close()
                    crate::models::csvio::exitstack_close(self, st, v, n, !exc);
                    crate::models::csvio::closing_exit(self, st, v, n);
                }
            }
        }
        for i in idx {
            if let Some(e) = self.unwind.finally_stack.get_mut(i) {
                e.2 = true;
            }
        }
    }

    /// A `try` statement (`stmt_try_parts` with its handlers and finally block).
    pub(super) fn stmt_try(&mut self, t: &'static ast::StmtTry, state: &mut State) {
        let fin = std::rc::Rc::new(Finally::Block(&t.finalbody));
        let handlers = t.handlers.iter().map(|h| {
            let ast::ExceptHandler::ExceptHandler(h) = h;
            let types = h.type_.as_deref().map(|ty| match ty {
                ast::Expr::Tuple(tup) => tup.elts.iter().collect(),
                other => vec![other],
            });
            Handler { types, name: h.name.as_ref().map(|n| n.id.as_str()), body: &h.body, line: frontend::node(h).line, maybe: false }
        });
        self.stmt_try_parts(&t.body, handlers.collect(), &t.orelse, fin, state);
    }

    /// A try statement from its parts (a with statement is one: its finally is `__exit__`).
    pub fn stmt_try_parts(&mut self, body: &'static [ast::Stmt], handlers: Vec<Handler>, orelse: &'static [ast::Stmt], fin: std::rc::Rc<Finally>, state: &mut State) {
        self.stmt_try_parts_rest(body, None, handlers, orelse, fin, state)
    }

    /// `stmt_try_parts`, whose body is the rest of a with statement's items when `rest` says
    /// (`with a, b: body` is `with a: with b: body`).
    pub(super) fn stmt_try_parts_rest(&mut self, body: &'static [ast::Stmt], rest: Option<(&'static ast::StmtWith, usize)>, handlers: Vec<Handler>, orelse: &'static [ast::Stmt], fin: std::rc::Rc<Finally>, state: &mut State) {
        let pushed = !fin.is_empty();
        if pushed {
            self.unwind.finally_stack.push((self.unwind.loop_stack.len(), fin.clone(), true));
        }
        let depth = self.unwind.finally_stack.len();
        self.stmt_try_inner(body, rest, &handlers, orelse, &fin, state);
        if pushed {
            self.unwind.finally_stack.truncate(depth - 1);
        }
    }

    /// The try itself: the body runs in a try context recording its raises; raises no handler
    /// surely catches go on (through the finally); each handler runs from those it may catch;
    /// the ways out (the body's end and `else`, each handler's end) are joined, then the
    /// finally runs on each way out.
    pub(super) fn stmt_try_inner(&mut self, body: &'static [ast::Stmt], rest: Option<(&'static ast::StmtWith, usize)>, handlers: &[Handler], orelse: &'static [ast::Stmt], fin: &std::rc::Rc<Finally>, state: &mut State) {
        // (a handler for a class catches its subclasses too: `enter_try`)
        let mut caught: FxHashSet<String> = handlers.iter().flat_map(|h| h.names()).collect();
        // handlers that all end by re-raising: the body's reads are still required
        let reraise = !handlers.is_empty() && handlers.iter().all(|h| matches!(h.body.last(), Some(ast::Stmt::Raise(_))));
        if reraise {
            caught.insert("!reraise".into());
        }
        // returns inside the try are collected apart from the earlier ones: the finally runs
        // on their way out
        let has_finally = !fin.is_empty();
        let outer_exits = if has_finally { state.exits.take() } else { None };
        let mut pre = state.copy();
        // the handlers' classes: which recorded raise each one catches (Python takes the
        // first handler that matches)
        let hclasses = self.handler_classes(handlers, &pre);
        self.enter_try(caught, &mut pre);
        let runs = self.runs_code(fin);
        if let Some(t) = self.unwind.tries.last_mut() {
            (t.hclasses, t.runs) = (hclasses.clone(), runs);
        }
        self.cx.cond_depth += 1;
        self.cx.branch_depth += 1;
        match rest {
            Some((w, i)) => self.stmt_with_from(w, i, state),
            None => self.exec_block(body, state),
        }
        let TryExit { mut recs, .. } = self.leave_try();
        // raises no handler surely catches go on to the enclosing try (or out of the call),
        // through the finally (unless a return / jump in it swallows them)
        let swallowing = !fin.is_empty() && fin.swallows();
        if !swallowing {
            self.reraise_uncaught(&mut recs, &hclasses, fin);
        }
        // where an exception no handler surely catches leaves the body, for a finally that
        // swallows it
        let escaping = |recs: &[crate::interp::RaiseRec]| State::join_all(recs.iter().filter(|r| !hclasses.iter().any(|hc| handler_matches(hc, &r.0) == Some(true))).map(|r| r.1.copy()).collect());
        let mut finally_escape = if swallowing { escaping(&recs) } else { None };
        if crate::interp::debug_level() == 2 {
            eprintln!("  try at L{}: raises recorded={} reachable={}", body.first().map(|s| frontend::node(s).line).unwrap_or(0), recs.len(), state.reachable);
        }
        let mut body_end = state.copy();
        let mut results: Vec<State> = Vec::new();
        // (a raise in the `else` or a handler passes the handlers by, not the finally: a try
        // context catching nothing takes it there, and the finally runs on it as on the body's)
        let wrap = has_finally && (!orelse.is_empty() || !handlers.is_empty());
        if wrap {
            self.enter_try(FxHashSet::default(), &mut pre.copy());
            if let Some(t) = self.unwind.tries.last_mut() {
                t.runs = runs;
            }
        }
        if !orelse.is_empty() && body_end.reachable {
            self.exec_block(orelse, &mut body_end);
        }
        results.push(body_end);
        self.run_handlers(handlers, &hclasses, &mut recs, &pre, &mut results);
        if wrap {
            let TryExit { recs: mut late, .. } = self.leave_try();
            if swallowing {
                let mut esc: Vec<State> = late.iter().map(|r| r.1.copy()).collect();
                esc.extend(finally_escape.take());
                finally_escape = State::join_all(esc);
            } else {
                self.reraise_uncaught(&mut late, &[], fin);
            }
        }
        self.cx.branch_depth -= 1;
        self.cx.cond_depth -= 1;
        let mut out = self.join_try_ways(body, orelse, handlers, results);
        if has_finally {
            self.finally_ways(fin, &mut out, outer_exits, finally_escape.take());
        } else if !fin.is_empty() {
            if !out.reachable {
                out = pre.copy();
            }
            self.run_finally(fin, &mut out, false);
        }
        state.replace_with(out);
    }

    /// The classes each handler names (with whether every name is a known exception class: a
    /// decided miss needs it).
    fn handler_classes(&mut self, handlers: &[Handler], pre: &State) -> Vec<(FxHashSet<String>, bool)> {
        handlers.iter().map(|h| {
            // (one that may catch anything: no class it surely catches or surely misses)
            if h.maybe {
                return (FxHashSet::default(), false);
            }
            let Some(elts) = &h.types else { return (h.names(), true) };
            // (no decided miss unless every name is a known exception class)
            let known = elts.iter().all(|e| {
                let n = dotted_last(e);
                let builtin = crate::interp::builtin_exc_names(&n);
                let user = matches!(e, ast::Expr::Name(_)) && {
                    let v = self.eval(e, &mut pre.copy());
                    v.kv.is_bot() && !v.pts.is_empty() && v.pts.iter().all(|o| self.heap.class_info_opt(o).is_some_and(|ci| ci.name == n))
                };
                user || (builtin.1 && builtin.0.len() > 1) || n == "BaseException"
            });
            (elts.iter().map(|e| dotted_last(e)).collect(), known)
        }).collect()
    }

    /// The exceptions of a raise (`x`) a handler of classes `hc` may bind: those of a class it
    /// does not surely miss (each class looked at once). None: only exceptions it misses.
    fn exceptions_caught(&self, x: &Value, hc: &(FxHashSet<String>, bool)) -> Option<Value> {
        if x.pts.is_empty() {
            return Some(x.clone());
        }
        let mut misses: rustc_hash::FxHashMap<ObjId, bool> = Default::default();
        let mut missed = |o: ObjId| {
            let miss = |an: &Analyzer| handler_matches(hc, &an.exc_value_names(&Value::obj(o))) == Some(false);
            match self.heap.instance_class(o) {
                Some(c) => *misses.entry(c).or_insert_with(|| miss(self)),
                None => miss(self),
            }
        };
        let keep: Vec<ObjId> = x.pts.iter().filter(|o| !missed(*o)).collect();
        if keep.len() == x.pts.len() {
            return Some(x.clone());
        }
        (!keep.is_empty() || !x.kv.is_bot()).then(|| Value::with_kv(crate::domain::ObjSet::from_vec(keep), x.kv.clone()))
    }

    /// Raises no handler surely catches go on to the enclosing try (or out of the call),
    /// through the finally.
    fn reraise_uncaught(&mut self, recs: &mut [crate::interp::RaiseRec], hclasses: &[(FxHashSet<String>, bool)], fin: &std::rc::Rc<Finally>) {
        // (raises no handler around surely catches leave the program through the finally
        // blocks: whatever their class, the finally runs once from their states joined)
        let leaving: Vec<usize> = (0..recs.len()).filter(|i| self.leaves_program(&recs[*i].0, hclasses)).collect();
        if leaving.len() > 1 && !fin.is_empty() {
            let mut j = crate::state::Joins::default();
            for i in &leaving {
                j.add(recs[*i].1.copy());
            }
            if let Some(mut st2) = j.finish() {
                (st2.reachable, st2.ret, st2.exits) = (true, None, None);
                self.run_finally(fin, &mut st2, true);
                if st2.reachable {
                    for i in &leaving {
                        self.reraise_rec((recs[*i].0.clone(), st2.copy(), recs[*i].2.clone()));
                    }
                }
            }
        }
        for (i, (cls, st, ev)) in recs.iter_mut().enumerate() {
            if hclasses.iter().any(|hc| handler_matches(hc, cls) == Some(true)) || (leaving.len() > 1 && !fin.is_empty() && leaving.contains(&i)) {
                continue;
            }
            let mut st2 = st.copy();
            if !fin.is_empty() {
                // (GeneratorExit raised at a yield runs code on its way out: `genclose.rs`)
                self.note_close_ran(cls);
                st2.reachable = true;
                st2.ret = None;
                st2.exits = None;
                self.run_finally(fin, &mut st2, true);
                if !st2.reachable {
                    continue;
                }
            }
            self.reraise_rec((cls.clone(), st2, ev.clone()));
        }
    }

    /// A raise of class `cls` that this try's handlers (`hclasses`) and every one around surely
    /// miss, no thread or future keeping it, no GeneratorExit at a yield: it leaves the program,
    /// its class matters to nothing on the way.
    fn leaves_program(&self, cls: &Option<crate::interp::ExcNames>, hclasses: &[(FxHashSet<String>, bool)]) -> bool {
        let misses = |hcs: &[(FxHashSet<String>, bool)]| hcs.iter().all(|hc| handler_matches(hc, cls) == Some(false));
        self.unwind.catching_depth == 0 && super::genclose::close_pos(cls).is_none() && !super::genclose::is_thrown(cls) && misses(hclasses) && self.unwind.tries.iter().all(|t| misses(&t.hclasses))
    }

    /// Each handler runs from the raises it may catch that no earlier one surely catches; its
    /// end state is one way out of the try.
    fn run_handlers(&mut self, handlers: &[Handler], hclasses: &[(FxHashSet<String>, bool)], recs: &mut [crate::interp::RaiseRec], pre: &State, results: &mut Vec<State>) {
        for (hi, h) in handlers.iter().enumerate() {
            // the exceptions raised at the recorded points (anything else: unknown)
            let unknown = Value::data(&format!("exc@L{}", h.line));
            let (mut closing, mut only_closing): (Option<Option<usize>>, bool) = (None, true);
            let (mut hs, ev) = {
                // the raises this handler may catch that no earlier one surely catches
                let mut e = State::bottom();
                let mut ev: Option<Value> = None;
                for (cls, st, x) in recs.iter_mut() {
                    if handler_matches(&hclasses[hi], cls) == Some(false) || hclasses[..hi].iter().any(|hc| handler_matches(hc, cls) == Some(true)) {
                        continue;
                    }
                    match super::genclose::close_pos(cls) {
                        Some(p) => {
                            closing = Some(match closing {
                                Some(q) if q != p => None,
                                _ => p,
                            })
                        }
                        None => only_closing = false,
                    }
                    // (a raise of one of several exceptions, `raise` of a handler's: the handler
                    // binds those of its classes; none of them: it catches none of this raise)
                    let Some(x) = self.exceptions_caught(x, &hclasses[hi]) else { continue };
                    let mut r = st.copy();
                    e = if e.reachable { e.join(&mut r) } else { r };
                    let x = if x.pts.is_empty() { unknown.clone() } else { x.clone() };
                    ev = Some(match ev { None => x, Some(p) => p.join(&x) });
                }
                (e, ev.unwrap_or(unknown))
            };
            if !hs.reachable {
                continue;
            }
            // (a raise in a call may carry its tests: those of the try's own path hold)
            hs.path = pre.path.clone();
            if let Some(name) = h.name {
                self.store_name(s(name), ev.clone(), &mut hs);
            }
            self.unwind.handling.push(h.names());
            self.unwind.handling_values.push(ev);
            if let Some(p) = closing {
                self.unwind.closing.push((self.cx.frames.len(), p, self.unwind.handling.len()));
                self.closing_caught(p, only_closing, &mut hs);
            }
            self.exec_block(h.body, &mut hs);
            if closing.is_some() {
                self.unwind.closing.pop();
            }
            self.unwind.handling_values.pop();
            self.unwind.handling.pop();
            results.push(hs);
        }
    }

    /// The ways out of a try (the body, each handler) joined; two that set a flag differently
    /// leave facts a later test of the flag uses.
    fn join_try_ways(&mut self, body: &'static [ast::Stmt], orelse: &'static [ast::Stmt], handlers: &[Handler], mut results: Vec<State>) -> State {
        // two ways out of the try (the body and one handler) that set a flag differently: a
        // later test of the flag tells which ran
        let two: Vec<usize> = (0..results.len()).filter(|i| results[*i].reachable).collect();
        let pair = if two.len() == 2 { Some((results[two[0]].copy(), results[two[1]].copy())) } else { None };
        let mut out = results.remove(0);
        for mut r in results {
            out = out.join(&mut r);
        }
        if let Some((a, b)) = pair {
            let mut assigned: Vec<intern::StrId> = Vec::new();
            for st in body.iter().chain(orelse.iter()) {
                collect_assigned(st, &mut assigned);
            }
            for h in handlers {
                h.body.iter().for_each(|x| collect_assigned(x, &mut assigned));
            }
            let vals: Vec<(intern::StrId, Option<Value>, Option<Value>, Option<Value>)> = assigned.iter().filter(|n| self.tested(**n) >= 1).map(|n| (*n, self.name_value(&a, *n), self.name_value(&b, *n), self.name_value(&out, *n))).collect();
            let new = name_facts(&vals, &a, &b, &out, self.frame().func.unwrap_or(0));
            add_facts(&mut out, new);
        }
        out
    }

    /// A try's finally on every way out: the normal end, the returns made inside it, an
    /// exception a `return` / jump in the finally swallows.
    fn finally_ways(&mut self, fin: &std::rc::Rc<Finally>, out: &mut State, outer_exits: Option<std::sync::Arc<State>>, finally_escape: Option<State>) {
        let mut finally_escape = finally_escape;
        let inner_exits = out.exits.take();
        let mut ret_acc = out.ret.take();
        let mut exits_acc: Option<State> = None;
        let add_exits = |e: Option<std::sync::Arc<State>>, acc: &mut Option<State>| {
            if let Some(e) = e {
                let mut s2 = e.copy();
                *acc = Some(match acc.take() {
                    None => s2,
                    Some(mut a) => a.join(&mut s2),
                });
            }
        };
        // the normal way out
        if out.reachable {
            self.run_finally(fin, out, false);
        }
        add_exits(out.exits.take(), &mut exits_acc);
        if let Some(r) = out.ret.take() {
            ret_acc = Some(match ret_acc { None => r, Some(a) => a.join(&r) });
        }
        // returns made inside the try: the finally runs before the function returns
        let mut ways: Vec<State> = Vec::new();
        if let Some(ie) = inner_exits {
            ways.push(ie.copy());
        }
        // an exception leaving the try (no handler here) that a `return` in the finally
        // swallows
        let n_exit_ways = ways.len();
        // (a `break` / `continue` in the finally swallows it as well)
        if let Some(esc) = finally_escape.take() {
            ways.push(esc);
        }
        for (wi, mut w) in ways.into_iter().enumerate() {
            let is_escape = wi >= n_exit_ways;
            w.reachable = true;
            w.ret = None;
            w.exits = None;
            self.run_finally(fin, &mut w, is_escape);
            if is_escape {
                // a finally that completes re-raises: nothing continues from here
                w.reachable = false;
            } else if w.reachable {
                w.record_exit();
            }
            add_exits(w.exits.take(), &mut exits_acc);
            if let Some(r) = w.ret.take() {
                ret_acc = Some(match ret_acc { None => r, Some(a) => a.join(&r) });
            }
        }
        add_exits(outer_exits, &mut exits_acc);
        out.exits = exits_acc.map(std::sync::Arc::new);
        out.ret = ret_acc;
    }
}

/// Does a handler (its class names, and whether they are all known exception classes) catch a
/// raise of this class: surely (Some(true)), surely not (Some(false)), or maybe (None)?
pub(in crate::interp) fn handler_matches(hc: &(FxHashSet<String>, bool), cls: &Option<crate::interp::ExcNames>) -> Option<bool> {
    if hc.0.contains("*") {
        return Some(true);
    }
    let (names, full) = cls.as_ref()?;
    if names.iter().any(|n| hc.0.contains(n)) {
        Some(true)
    } else if *full && hc.1 {
        Some(false)
    } else {
        None
    }
}
