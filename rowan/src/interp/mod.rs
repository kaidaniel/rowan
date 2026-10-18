//! The abstract interpreter: an `Analyzer` walks the program's syntax tree on abstract
//! `State`s. The `Analyzer` is a set of components, each in its own file with its own state
//! (the struct's fields below); the walk itself (statements, expressions, calls, accesses) is
//! `impl Analyzer` across the sibling files, one concern per file. `README.md` in this folder
//! is the map of the files and where to start reading.

use crate::domain::Value;
use crate::frontend::{self, Node};
use crate::intern;
use crate::state::{reset_thread_state, Kind, State};

// The components: each owns one field of `Analyzer`.
pub mod budget;
pub mod context;
pub mod effects;
pub mod entry;
pub mod findings;
pub mod gens;
pub mod heap;
pub mod names;
pub mod program;
pub mod recursion;
pub mod tries;
pub mod unknown;

// The walk: statements, expressions and what they share.
pub mod access;
pub mod annotations;
pub mod alloc;
pub mod attrs;
pub mod calls;
pub mod classes;
pub mod compare;
pub mod comps;
pub mod exprs;
pub mod flows;
pub mod funcs;
pub mod gc;
pub mod guards;
pub mod items;
pub mod iter;
pub mod keys;
pub mod library;
pub mod loops;
pub mod narrow;
pub mod ops;
pub mod stmts;

// What the models, the report and the driver see.
pub mod api;
pub mod facts;
pub mod render;
pub mod report;

pub use api::{Args, Call};
pub use budget::Budget;
pub use context::{Context, Frame, FrameKind, Modes, PreCall};
pub use entry::Entries;
pub use findings::*;
pub use gens::*;
pub use heap::*;
pub use names::Names;
pub use program::{package_root_of_file, Program};
pub use recursion::Recursion;
pub use tries::*;
pub use unknown::{any_object, Havoc};
pub use crate::models::exceptions::builtin_exc_names;
pub use crate::models::tables::*;

/// A `for` over at most this many known items is unrolled (each iteration straight-line code);
/// a comprehension that calls nothing per item, up to four times as many (`comps.rs`).
pub const UNROLL_MAX: usize = 16;
/// Past `UNROLL_MAX`, known items that are each one distinct object are unrolled up to this many
/// (`distinct_objects`).
pub const RECEIVERS_MAX: usize = 256;
/// A call of a value that may be more than this many of the program's functions is one call
/// none of which runs (`budget_skipped`).
pub const PICK_MAX: usize = 64;
/// How many keys deep below an open input a read makes placeholders (`path_placeholder_value`);
/// deeper reads are unknowns.
pub const PATH_DEPTH: u32 = 3;
/// A read on a value of more objects than this shares its placeholders (`Modes::wide_read`).
pub const WIDE_READ: usize = 8;
/// Call sites kept in an allocation context (deeper chains: a hash of the prefix).
pub const CHAIN: usize = 2;
/// Statements a function code may run in all its calls (`ROWAN_LIMITS=budget=N`): past them
/// its calls are unknown code.
pub const FN_BUDGET: usize = 4096;
/// A safety net, not the bound on work (the budget is): past this many milliseconds
/// (`ROWAN_LIMITS=ms=N`) calls of user code are unknown calls and running bodies stop.
pub const DEADLINE_MS: usize = 10000;

/// A bound from `ROWAN_LIMITS=budget=N,ms=N,fanout=N` (the only knobs: `FN_BUDGET`,
/// `DEADLINE_MS`, `budget::FANOUT`),
/// else `default`. Read once per thread.
pub fn limit(name: &str, default: usize) -> usize {
    thread_local! {
        static LIMITS: std::collections::HashMap<String, usize> = {
            let mut m = std::collections::HashMap::new();
            if let Ok(v) = std::env::var("ROWAN_LIMITS") {
                for part in v.split(',') {
                    if let Some((k, n)) = part.split_once('=') {
                        if let Ok(n) = n.trim().parse::<usize>() {
                            m.insert(k.trim().to_string(), n);
                        }
                    }
                }
            }
            m
        };
    }
    LIMITS.with(|m| m.get(name).copied().unwrap_or(default))
}

/// `ROWAN_DEBUG` as a number (0 when unset), read once.
pub fn debug_level() -> u8 {
    static LEVEL: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *LEVEL.get_or_init(|| match std::env::var("ROWAN_DEBUG") {
        Ok(v) if v == "2" => 2,
        Ok(v) if !v.is_empty() && v != "0" => 1,
        _ => 0,
    })
}

/// Is `ROWAN_DEBUG` set (body runs, skipped calls and the `work:` line on stderr)?
pub fn debug_enabled() -> bool {
    debug_level() > 0
}

/// A node's position as a site label: `L5:3-12` in the main module, `M2:L5:3-12` in others.
pub fn site_string(n: &Node) -> String {
    format!("{}L{}:{}-{}", frontend::site_prefix(n.module), n.line, n.col, n.end_col)
}

/// The analyzer: the components below. Everything a path knows is in its `State`; what the
/// components hold is the rest, and falls in four classes — what a call memo keyed by the
/// entry state would have to treat apart (a hit skips the body run):
///
/// * the **context** of the running code, saved and restored around each call: `cx` (frames,
///   callers' names, call chain, branch depth, loop recency), `mode`, `recursion` (the running
///   activations), `unwind` (exceptions in flight, running tries). Part of a memo key, beyond
///   the state and the arguments: `Frame::ctx` / `single` / `unroll`, the call chain, the
///   branch depth.
/// * **flow-insensitive state a run changes**, which a hit would have to replay: `heap` (the
///   objects a run makes, their payloads and fallback lengths, class attributes, property
///   setters, copy roots, keys standing for objects), `m` (the library models' tables),
///   `program` (modules imported, classes defined, `TYPE_CHECKING` names), `entry` (functions
///   a call ran, closures a run made), `budget` (statements charged, functions refused), `gens`
///   (lazy generators, per-yield states and effects, watched runs), the raise states recorded
///   into the callers' tries (`unwind`), and the code closures of `effects` (computed once, from
///   the first state that asks).
/// * **output** a run appends, replayable as recorded: `findings` (verdicts, which a superseded
///   round drops again, notes, summaries, row checks, hover facts) and `names` (which the torch
///   model also reads back: an unknown that is a call's result).
/// * **caches**, safe to skip: `havoc` (the memo of walks; its marks and counters), and
///   `effects`' per-code name uses.
#[derive(Default)]
pub struct Analyzer {
    /// the library models' own tables (`models::Tables`)
    pub m: crate::models::Tables,
    /// the main file, import paths, models, modules imported (`interp/program.rs`)
    pub(in crate::interp) program: Program,
    /// the objects (`interp/heap.rs`)
    pub heap: Heap,
    /// what the analysis found: verdicts, notes, summaries, golden row checks, hover facts
    /// (`interp/findings.rs`)
    pub(in crate::interp) findings: Findings,
    /// the report's names for unknown objects and calls (`interp/names.rs`)
    pub(in crate::interp) names: Names,
    /// unknown code's havoc walks (`interp/unknown.rs`)
    pub(in crate::interp) havoc: Havoc,
    /// what code may change when it does not run (`interp/effects.rs`)
    pub(in crate::interp) effects: effects::EffectsCache,
    /// budgets, the deadline and the work counters (`interp/budget.rs`)
    pub(in crate::interp) budget: Budget,
    /// the running activations, recursion's fixpoints (`interp/recursion.rs`)
    pub(in crate::interp) recursion: Recursion,
    /// entry runs: which functions had one, their receivers (`interp/entry.rs`)
    pub(in crate::interp) entry: Entries,
    /// generators and `@contextmanager` bodies run at their consumers (`interp/gens.rs`)
    pub(in crate::interp) gens: Gens,
    /// the ways out of running code: raises, try contexts, handlers, break / continue, finally
    /// (`interp/tries.rs`)
    pub(in crate::interp) unwind: Unwind,
    /// the context the running code is in: frames, callers' names, call chain, branch depth
    /// (`interp/context.rs`)
    pub(in crate::interp) cx: Context,
    /// modes an evaluation sets for the code it calls (`interp/context.rs`)
    pub(in crate::interp) mode: Modes,
    /// `None`
    pub none_v: Value,
}

impl Analyzer {
    /// An analyzer for the program whose main file is `filename` (its text `source`): imports
    /// searched in `search_paths` (none unless `follow_imports`), entry runs of the functions
    /// the program does not call unless `analyse_entry_points` is false. `run` runs it.
    pub fn new(source: String, filename: &str, search_paths: Vec<String>, analyse_entry_points: bool, follow_imports: bool) -> Result<Analyzer, String> {
        reset_thread_state();
        crate::models::pynum::set_opaque(false);
        let (idx, tree) = frontend::parse("__main__", Some(filename), source)?;
        frontend::set_main(idx);
        let program = Program::new(filename, idx, if follow_imports { search_paths } else { Vec::new() }, analyse_entry_points);
        let mut an = Analyzer {
            program,
            none_v: Value::lit(intern::key_none()),
            ..Default::default()
        };
        an.program.main_module = an.make_module("__main__", Some(filename), idx, tree);
        an.m.series = an.alloc_global(Kind::Series, "builtin:Series", "Series", Payload::None);
        Ok(an)
    }

    /// The running frame (innermost).
    pub fn frame(&self) -> &Frame {
        self.cx.frames.last().expect("no frame")
    }
    pub fn frame_mut(&mut self) -> &mut Frame {
        self.cx.frames.last_mut().expect("no frame")
    }

    /// Past the deadline a statement in a function body does not run: its path ends, and
    /// the call it belongs to is replaced by an unknown call (`call_user`). Module code goes on
    /// (its calls are unknown calls). Checked at every statement.
    pub(in crate::interp) fn cut_body(&mut self, state: &mut State) -> bool {
        if !self.budget.tick() {
            return false;
        }
        if self.cx.frames.last().is_some_and(|f| f.kind == FrameKind::Function) {
            state.reachable = false;
            return true;
        }
        false
    }
}
