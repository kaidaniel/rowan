//! The context the running code is in: the frames and the callers' names, the call chain,
//! how deep in branches and loops it is, the running comprehensions and loop iterations, and
//! the model calls running. All of it is dynamic: pushed and popped (or saved and restored)
//! by the code that enters a frame, a branch, a loop or a model call.
//!
//! For a call memo: this is what a call's result may depend on beyond its entry `State` and
//! its arguments (the allocation context: `Frame::ctx`, `single`, `unroll`; the call chain the
//! verdicts are recorded under; `cond_depth` / `branch_depth` for requirements), so the part
//! of a memo key outside the state.

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use super::Frame;
use crate::domain::{ObjId, Value};
use crate::intern::{KeyId, StrId};
use crate::state::Kind;

#[derive(Default)]
pub struct Context {
    /// the running frames (innermost last)
    pub frames: Vec<Frame>,
    /// locals of the callers of the running call (innermost last), for liveness
    pub env_stack: Vec<IndexMap<StrId, Value>>,
    /// the call sites (lines) of the running calls, innermost last (the verdicts' chain)
    pub call_chain: Vec<u32>,
    /// branches and loops the running code is inside of (`if` / `try` / boolean operators /
    /// loops): a requirement recorded inside one is "sometimes"
    pub cond_depth: i32,
    /// like `cond_depth` without loops: a requirement recorded under an `if`/`try`/boolean
    /// operator is "sometimes"; one inside a loop body is per element (always)
    pub branch_depth: i32,
    /// per comprehension generator level: whether its items are known (unrolled)
    pub comp_known: Vec<bool>,
    /// per running comprehension: the cells of its variables (closed over by functions made
    /// inside it, which see the variable's latest value)
    pub comp_cells: Vec<FxHashMap<StrId, ObjId>>,
    /// recency abstraction: per loop iteration being analysed (innermost last), the recent
    /// objects allocated in it: (recent oid, kind, site, ctx)
    pub recent_stack: Vec<Vec<(ObjId, Kind, String, String)>>,
    /// a context part every allocation adds (a model run once per receiver)
    pub alloc_extra: Option<String>,
    /// model calls running (`run_model`), the outermost one's number, and the objects
    /// allocated in it
    pub model_depth: u32,
    pub model_epoch: u64,
    pub model_born: FxHashMap<ObjId, u64>,
    /// instances whose `__init__` is running for their construction (stores into them are
    /// strong)
    pub fresh_init: FxHashSet<ObjId>,
}

/// Modes an evaluation sets for the code it calls, restored after: how an access reports, how
/// a store goes, what a test evaluation records.
#[derive(Default)]
pub struct Modes {
    /// the verdict being recorded is on a guarded access
    pub verdict_guarded: bool,
    /// the access being made is on a value that may also be a scalar (a str, a number): no
    /// object's lack of the key makes it fail for sure
    pub recv_scalar: bool,
    /// (canonical key, key as written) while an access with a numerically-equal key runs
    pub key_alias: Option<(KeyId, KeyId)>,
    /// inside `object.__setattr__`: a plain store even when the class defines `__setattr__`
    pub bypass_setattr: bool,
    /// expressions in call position (`x.m` of `x.m(…)`): a method read of an input is not a
    /// key of it
    pub callee_nodes: FxHashSet<usize>,
    /// while a test is evaluated (`split`): its frame depth and the value each node of it had
    pub trace: Option<(usize, FxHashMap<usize, Value>)>,
    /// the values the test being narrowed had (`split`, read by `traced`)
    pub traced: FxHashMap<usize, Value>,
    /// the comprehension written as the first argument of `torch.cat` / `torch.stack` being
    /// called (its address): those raise on no tensors, so it ran at least once
    pub nonempty_comp: usize,
    /// set by a comprehension above for its generator: at least one iteration ran
    pub nonempty_gen: bool,
    /// whether the last `loop_fixpoint` converged (its last round ran from the fixpoint)
    pub loop_converged: bool,
}
