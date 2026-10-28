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

use crate::domain::{ObjId, Row, Value};
use crate::intern::{KeyId, StrId};
use crate::state::{Kind, State};

/// What code a frame runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameKind {
    Module,
    Function,
    Class,
}

/// One running frame: a module's code, a function's call, a class body. Names live in the
/// `State` (`env`); the frame says how to look them up and how to allocate.
#[derive(Debug)]
pub struct Frame {
    pub kind: FrameKind,
    /// its scope in its module's `Scopes` (which names are locals, cells, globals)
    pub scope: usize,
    /// the module whose globals it reads
    pub module: ObjId,
    /// the allocation context of the objects it makes: the call sites of the running calls
    /// (`Analyzer::callee_site`, `receiver_ctx`)
    pub ctx: String,
    /// it runs once per runtime call here (no loop or several-receiver call around it): its
    /// allocations are single objects
    pub single: bool,
    /// `single` ignoring loops: with the recency abstraction an allocation inside a loop is
    /// a strong "recent" object when this holds
    pub single_nl: bool,
    /// the function it runs (None at module level and in a class body)
    pub func: Option<ObjId>,
    /// the function's code (`FuncNode::addr`), whose statements count against its budget
    pub code: usize,
    /// its cells and the closure's: name → cell object
    pub cells: IndexMap<StrId, ObjId>,
    /// a class body's class
    pub class_oid: Option<ObjId>,
    /// the variables of the running comprehensions (innermost last): their values are in the
    /// state's names, the enclosing ones' put back when the comprehension ends
    pub comp_names: Vec<Vec<StrId>>,
    /// loops it is inside of
    pub loop_depth: u32,
    /// the qualified name of what it runs (nested functions' qualified names start with it)
    pub label: String,
    /// indices of the enclosing unrolled loop iterations (part of the call context)
    pub unroll: Vec<usize>,
    /// a call: the objects allocated from here on are its own, and the state before
    /// it (a `return` drops its own objects nothing can reach any more)
    pub pre_call: Option<PreCall>,
    /// the call's own recent objects (object, kind, and its summary's site and context): made
    /// at a site whose object is a summary, folded into it when the call returns (`call_recent`)
    pub recent: Vec<(ObjId, Kind, String, String)>,
}

/// The start of a call: the heap then and the state before it (`Frame::pre_call`).
#[derive(Clone)]
pub struct PreCall(pub super::heap::HeapMark, pub std::rc::Rc<State>);

impl std::fmt::Debug for PreCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PreCall({})", self.0.serial)
    }
}

impl Frame {
    /// A frame with no function, cells or class.
    pub fn new(kind: FrameKind, scope: usize, module: ObjId, ctx: &str, single: bool, label: &str) -> Frame {
        Frame {
            kind,
            scope,
            module,
            ctx: ctx.to_string(),
            single,
            single_nl: single,
            func: None,
            code: 0,
            cells: IndexMap::new(),
            class_oid: None,
            comp_names: Vec::new(),
            loop_depth: 0,
            label: label.to_string(),
            unroll: Vec::new(),
            pre_call: None,
            recent: Vec::new(),
        }
    }
}

/// The context the running code is in (see the module doc).
#[derive(Default)]
pub struct Context {
    /// the running frames (innermost last)
    pub frames: Vec<Frame>,
    /// locals of the callers of the running call (innermost last), for liveness
    pub env_stack: Vec<IndexMap<StrId, Value>>,
    /// their possibly-unbound names then (`State::unbound`), beside `env_stack`
    pub unbound_stack: Vec<std::sync::Arc<Vec<StrId>>>,
    /// the call sites (lines) of the running calls, innermost last (the verdicts' chain)
    pub call_chain: Vec<u32>,
    /// the modules of `call_chain`'s sites
    pub call_files: Vec<u32>,
    /// branches and loops the running code is inside of (`if` / `try` / boolean operators /
    /// loops): a requirement recorded inside one is "sometimes"
    pub cond_depth: i32,
    /// like `cond_depth` without loops: a requirement recorded under an `if`/`try`/boolean
    /// operator is "sometimes"; one inside a loop body is per element (always)
    pub branch_depth: i32,
    /// per comprehension generator level: whether its items are known (unrolled)
    pub comp_known: Vec<bool>,
    /// filters not decided for the comprehension item being run (it may be dropped)
    pub comp_maybe: usize,
    /// generator levels of the running comprehension that iterate a set of several items (in
    /// an order Python does not promise: the result has no positions)
    pub comp_unordered: usize,
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
    /// the call of the program's code starting is a model's (its arguments are the library's
    /// to choose: no TypeError for ones that do not fit)
    pub called_by_model: bool,
    pub model_epoch: u64,
    pub model_born: FxHashMap<ObjId, u64>,
    /// instances whose `__init__` is running for their construction (stores into them are
    /// strong), with what raises found of them so far (`drop_unreached`)
    pub fresh_init: FxHashMap<ObjId, Building>,
    /// per running summary comprehension (`[Block() for _ in range(n)]`: its iterations
    /// cannot be told apart; by the object it makes): the number of items its one iteration
    /// stands for
    pub summary_items: FxHashMap<ObjId, usize>,
    /// running calls of one of several functions or classes a value picks (`CONVERTERS[k](x)`
    /// with `k` not known): the call as written and the one running. Rowan cannot tell which
    /// one the program picks for the arguments it is given: what fails in one fails where the
    /// program picks it (`possibly`), not on every path (`missing`)
    pub picked: Vec<(String, String)>,
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
    /// the access being made is on one of several receivers (`per_receiver`): an object that
    /// answers it through `__getattr__` says so, against those that lack it
    pub one_of_several: bool,
    /// (canonical key, key as written) while an access with a numerically-equal key runs
    pub key_alias: Option<(KeyId, KeyId)>,
    /// inside `object.__setattr__`: a plain store even when the class defines `__setattr__`
    pub bypass_setattr: bool,
    /// a `get` reads the item: a defaultdict's factory and a Counter's 0 do not answer it
    pub plain_get: bool,
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
    /// the generator expression given alone to a library function that takes all its items at
    /// once (its address): it runs where it is made (`expr_generator`)
    pub bulk_genexp: usize,
    /// whether the last `loop_fixpoint` converged (its last round ran from the fixpoint)
    pub loop_converged: bool,
    /// a read on a value of more than `WIDE_READ` objects (its frame depth, site): what it
    /// finds in their open rows is one placeholder per key there, not one per object (weak; a
    /// library object's attribute: one unknown per name)
    pub wide_read: Option<(usize, String)>,
    /// a module's attribute read binds a deferred definition as it is (`from m import f` in a
    /// module's code, `deferred.rs`)
    pub keep_deferred: bool,
    /// the rows of the objects whose parts its placeholders stand for (joined at its end:
    /// one by one, each join cost the size of all before)
    pub wide_rows: Vec<(ObjId, Row)>,
}

/// An instance under construction, as raises see it (`Analyzer::drop_unreached`): the heap's
/// serial when it began, the state at the last look, what older objects' slots written since it
/// began held, the objects whose slots written since hold it, how many raises looked, and
/// whether a raise found it held.
pub struct Building {
    pub mark: u64,
    /// the heap at the last look: objects given a payload since are looked at in the next
    pub seen: super::heap::HeapMark,
    /// an object made before it began was given a payload holding objects since
    pub repaid: bool,
    pub at: State,
    pub roots: Vec<ObjId>,
    pub holders: Vec<ObjId>,
    pub looks: u32,
    pub held: bool,
}
