//! Consumers that stop early: `any`, `all`, `next` and `x in g` take a generator's items only
//! until the answer is known. A generator made for one of them alone (`any(f(x) for x in xs)`,
//! `next(gen(), None)`, `k in gen()`: written as the argument, not run yet) runs with the
//! consumer at its yields (`StopHook`): at each one the answer splits the state into the paths
//! where it is decided — the generator stops there — and those where it goes on to its next item.
//! Where it stops, CPython closes it when the consumer drops it: GeneratorExit is raised at
//! that yield (its `finally` blocks and `__exit__`s run), caught where the consumer is.
//! Other generators run whole, as consumers take them (`force_gen`).

use ruff_python_ast as ast;
use rustc_hash::FxHashSet;

use super::{Analyzer, Args};
use crate::domain::Value;
use crate::frontend;
use crate::intern;
use crate::state::{Kind, State};

/// What a consumer that stops early asks of each item.
#[derive(Clone)]
pub enum Stop {
    /// `any`: stops at a true item
    Any,
    /// `all`: stops at a false item
    All,
    /// `next(g[, default])`: stops at the first item
    Next(Option<Value>),
    /// `x in g`: stops at an item equal to x
    In(Value),
}

/// A consumer that stops early, running the generator made for it (one frame above `depth`).
pub struct StopHook {
    pub depth: usize,
    pub stop: Stop,
    /// the generator as written (the argument, the right operand of `in`)
    pub expr: &'static ast::Expr,
    /// the generator's body began to run (a recursive or skipped call does not)
    pub entered: bool,
    /// the consumer's answers where it stopped, joined (None: it stopped nowhere)
    pub result: Option<Value>,
}

fn bool_v(b: bool) -> Value {
    Value::lit(intern::key_bool(b))
}

impl Analyzer {
    /// `any(g)`, `all(g)`, `next(g)`, `next(g, d)` as written, of a generator made for the call
    /// alone: its answer (None: not such a call; the callee's model takes the generator).
    pub(super) fn stop_early_call(&mut self, fv: &Value, call: Option<&'static ast::ExprCall>, args: &[Value], a: &Args, state: &mut State) -> Option<Value> {
        let c = call?;
        let f = fv.single().filter(|f| fv.kv.is_bot() && self.heap.kind(*f) == Kind::External)?;
        if !a.kw.is_empty() || !a.star.is_empty() || !a.star_kw.is_empty() || c.arguments.args.len() != args.len() {
            return None;
        }
        let stop = match (self.heap.external_qual(f).as_str(), args.len()) {
            ("builtins.any", 1) => Stop::Any,
            ("builtins.all", 1) => Stop::All,
            ("builtins.next", 1) => Stop::Next(None),
            ("builtins.next", 2) => Stop::Next(Some(args[1].clone())),
            _ => return None,
        };
        self.stop_early(stop, &args[0], &c.arguments.args[0], state)
    }

    /// The consumer `stop` of generator `g`, made by `expr` and by nothing else, not run yet: its
    /// body runs with the consumer at its yields. None (nothing run): not such a generator, or
    /// one whose body did not run (its consumer takes it as unknown code's).
    pub(super) fn stop_early(&mut self, stop: Stop, g: &Value, expr: &'static ast::Expr, state: &mut State) -> Option<Value> {
        let o = g.single().filter(|_| g.kv.is_bot() && state.reachable)?;
        let lg = self.gens.lazy.get(&o).cloned()?;
        if lg.node.addr != frontend::node(expr).addr || super::gens::lazy_state(state, o) != (true, false) {
            return None;
        }
        let resumes = match self.heap.func_info(lg.foid).node {
            super::FuncNode::Def(d) => crate::syntax::stop_early_shape(&d.body)?,
            _ => false,
        };
        self.gens.force_at = Some((lg.site.clone(), lg.ctx.clone()));
        let saved = std::mem::replace(&mut self.cx.call_chain, lg.chain.clone());
        let saved_files = std::mem::replace(&mut self.cx.call_files, lg.files.clone());
        let mut pre = state.copy();
        self.enter_try(FxHashSet::from_iter(["GeneratorExit".to_string()]), &mut pre);
        self.gens.stop_hooks.push(StopHook { depth: self.cx.frames.len(), stop: stop.clone(), expr, entered: false, result: None });
        let r = self.call_user(lg.foid, lg.self_v.clone(), Args::new(&lg.args, &lg.kwargs, &[], &[]), state, &lg.node, false);
        // (the answers where it stopped are the collector's roots until taken: `gc_pinned`)
        let exit = self.leave_try();
        let hook = self.gens.stop_hooks.pop().expect("stop hook");
        self.cx.call_chain = saved;
        self.cx.call_files = saved_files;
        // (where it stopped and was closed: the consumer goes on from there)
        let mut closed: Option<State> = None;
        for rec in exit.recs {
            match &rec.0 {
                Some((names, _)) if names.first().is_some_and(|n| n == "GeneratorExit") => {
                    let mut st = rec.1;
                    closed = Some(match closed {
                        Some(mut c) => c.join(&mut st),
                        None => st,
                    });
                }
                _ => self.reraise_rec(rec),
            }
        }
        let ran = hook.entered && r.pts.contains(o);
        // the answer where it ran to its end (where its code went on after GeneratorExit, too)
        let mut result = hook.result.clone().unwrap_or_default();
        if state.reachable && ran {
            match &stop {
                Stop::Any | Stop::In(_) => result = result.join(&bool_v(false)),
                Stop::All => result = result.join(&bool_v(true)),
                Stop::Next(Some(d)) => result = result.join(d),
                Stop::Next(None) if resumes && hook.result.is_some() => self.may_raise_here("StopIteration", state),
                Stop::Next(None) => self.raise_here("StopIteration", state),
            }
        }
        if let Some(mut c) = closed {
            let j = if state.reachable { state.join(&mut c) } else { c };
            state.replace_with(j);
        }
        state.slots.remove(&o, &(crate::domain::Ns::Attr, intern::key_str_lit("\u{1}lazyargs")));
        state.set_slot(o, crate::domain::Ns::Attr, intern::key_str_lit("\u{1}lazy"), Value::lit(intern::key_int(0)), true);
        if !ran {
            self.gen_body_not_run(o, &r, &lg.node, state);
            return None;
        }
        Some(result)
    }

    /// Does a consumer that stops early run the generator whose activation begins here? Marks
    /// it begun.
    pub(super) fn stop_hook_begins(&mut self) -> bool {
        let depth = self.cx.frames.len();
        match self.gens.stop_hooks.last_mut() {
            Some(h) if !h.entered && h.depth + 1 == depth => {
                h.entered = true;
                true
            }
            _ => false,
        }
    }

    /// Is a yield here one of the generator a consumer that stops early runs?
    pub(super) fn at_stop_hook(&self) -> bool {
        self.gens.stop_hooks.last().is_some_and(|h| h.entered && h.depth + 1 == self.cx.frames.len())
    }

    /// The generator yields `v` to a consumer that stops early: where the consumer's answer is
    /// decided, it stops (GeneratorExit raised there, the consumer's answer kept); the state goes
    /// on where it is not.
    pub(super) fn stop_at_yield(&mut self, v: Value, state: &mut State) {
        let (stop, expr) = {
            let h = self.gens.stop_hooks.last().expect("stop hook");
            (h.stop.clone(), h.expr)
        };
        let nd = frontend::node(expr);
        let (decided, answer) = match &stop {
            Stop::Any => (self.truth_of(&v, state, &nd), bool_v(true)),
            Stop::All => (self.truth_of(&v, state, &nd).map(|t| !t), bool_v(false)),
            Stop::Next(_) => (Some(true), v),
            Stop::In(x) => {
                let eq = self.compare(ast::CmpOp::Eq, &v, x, expr, &nd, state);
                (self.truth_of(&eq, state, &nd), bool_v(true))
            }
        };
        if !state.reachable || decided == Some(false) {
            return;
        }
        let mut here = if decided == Some(true) { std::mem::replace(state, State::bottom()) } else { state.copy() };
        if let Some(h) = self.gens.stop_hooks.last_mut() {
            h.result = Some(match h.result.take() {
                Some(r) => r.join(&answer),
                None => answer,
            });
        }
        self.may_raise_here("GeneratorExit", &mut here);
    }
}
