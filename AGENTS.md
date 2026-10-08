# AGENTS.md — rowan, handoff notes

Static key-set inference for Python records: infers, for every dict-like value (dicts,
DataFrame columns, object attributes, kwargs, JSON/CSV rows, argparse namespaces…), which
keys it has at each point and which keys the program demands of it; demands on external
inputs become inferred schemas. Design: `SPECIFICATION.md` (17 sections, Appendix A golden
cases; §9.8 lists how the prototype departs from the text). User-facing summary: `README.md`.
This file is for whoever picks the work up.

## State of the code

Two engines, one behaviour:

* `rowan/` — the Phase-0 Python prototype (~8.5k lines), **frozen as the oracle**. Not
  changed by the port except `tests/run_golden.py` (see below).
* `rowan-rs/` — the Rust engine (~14k lines), a port of the prototype that reproduces it
  exactly in `--compat` mode and adds the algorithmic improvements of the old "next steps"
  in its default mode. This is where development continues.

| file | role |
| --- | --- |
| `rowan-rs/src/intern.rs` | thread-local interners: keys (`Key` → `KeyId`), strings, sources, lenses; `IdSet<T>` = sorted `Arc<[T]>` with merge-based set ops (`KeySet`, `ObjSet`, …). |
| `rowan-rs/src/domain.rs` | port of `domain.py`: lens steps, `Source`, `Row` (`Arc` + cached hash, all sets as `IdSet`s), `classify`, `KeyVal`, `ReqInfo`, `Value`, `chain_extend`. |
| `rowan-rs/src/layered.rs` | `LayerMap` / `TwoLevel` as in `layered.py` (frozen layers, O(1) copy, O(diff) join/same/diff_keys) plus `copy_ref` (copy from a shared reference, used for `exits`); randomised tests against a naive model. |
| `rowan-rs/src/state.rs` | `Kind`, `State` (env, narrowed, rows, slots, elems, reqs, ret, exits as `Arc<State>`, symbolic), `Footprint`. |
| `rowan-rs/src/scopes.rs` | lexical scopes (locals / cells / free / globals) over the ruff AST. |
| `rowan-rs/src/frontend.rs` | parsing (`ruff_python_parser`), leaked `&'static` module ASTs, a thread-local registry mapping any node address to its module so a node carried across modules (a deferred access in a summary) still knows its file and line; `Node` = located node (synthetic nodes for the prototype's fake calls). |
| `rowan-rs/src/interp/mod.rs` | `Analyzer` struct, `Options`, `Obj`/`Payload` (typed replacement of the prototype's `payload: Any`), `FuncInfo`/`ClassInfo` (`Rc`, `Cell` for the mutable flags), allocation (`alloc`, `alloc_raw`, `alloc_in_frame`), verdicts, entry points, `rename_recent`. |
| `rowan-rs/src/interp/{stmts,exprs,funcs,narrow,access,calls}.rs` | statements, expressions and comprehensions, functions / names / targets / iteration, narrowing, record access and attributes, calls and bindings — method by method as in `interp.py`. |
| `rowan-rs/src/summary.rs` | port of `summary.py`: placeholders, `SymReq`/events, `summarise` (rounds, partial summaries, now also provisional summaries for SCCs), `Inst` (the instantiation closures of `_instantiate` as a struct with its caches), `focus_groups` (allocation-site resolution). |
| `rowan-rs/src/models/{mod,pandas}.rs` | library models; `Registry::lookup` with the same prefix / allowlist fallbacks. |
| `rowan-rs/src/report.rs` | dedupe, schema table, text, JSON lines, and the `--golden` JSON document the harness consumes. |
| `rowan-rs/src/main.rs` | CLI; runs the analysis on a thread with a 1 GiB stack. |
| `rowan-rs/py/` | PyO3 binding (`rowan_rs.analyse(source, …)` / `analyse_file(path)` → `--golden` JSON string). Builds with maturin; see known problems. |
| `vscode-rowan/` | VS Code extension (plain JS, no build): runs `rowan-rs FILE --facts` on open/save, hover = facts for (line, name), verdicts as diagnostics; `node test.js` checks the rendering against the engine. `--facts` (`report::facts_json`) records, at every `Name` load/store in the main module, the value and the rows of its objects, joined per (line, name); facts from symbolic summary bodies are flagged `symbolic` (rows relative to the function's entry). |
| `tests/run_golden.py` | golden harness; `--rust`, `--both`, `--engine=…`; a case may be restricted to one engine with a `#@engine rust` comment (used by `f37_loop_recency.py`, which documents behaviour the prototype does not have). `ROWAN_COMPAT=1` passes `--compat` to the Rust engine. |
| `tests/diff_engines.py` | differential check (spec §15.4, static half): dedupe'd verdict sets and schema tables must be identical between the oracle and `rowan-rs --compat`. |

Run (Python via `uv`; `.python-version` pins 3.14 — the system `python3` is 3.9 and cannot
parse `match`):

```
cd rowan-rs && cargo build --release && cargo test --release --lib
rowan-rs/target/release/rowan-rs FILE.py [--json|--golden|--facts] [--no-rows --no-summaries --no-entry --no-imports --compat]
uv run --no-project python tests/run_golden.py --rust     # 63/63 (default mode)
ROWAN_COMPAT=1 uv run --no-project python tests/run_golden.py --rust   # 36/36: the prototype's cases, prototype semantics
uv run --no-project python tests/run_golden.py            # Python oracle, 36/36
uv run --no-project python tests/diff_engines.py          # 43 programs identical between oracle and rowan-rs --compat
ROWAN_SUMMARIES=0 …                                       # old per-call-site engine (34/36 in both engines, same two cases)
```

Golden cases: `a01`–`e36` are the prototype's (both engines); `f37` and `g40`–`g65` are
`#@engine rust` cases written against the Rust engine (AST node coverage, closures taking
closures returning lambdas, reassignment, control flow, inputs indexing results of input
functions, classes, string keys, containers, guards, modules, generators, recursion, kwargs
forwarding, configs, pandas, annotations, mutation through layers, comprehensions, dispatch,
a CLI script, dict methods, a larger mixed program).  Harness directives: `#@ tier key`,
`#@row`, `#@schema`, plus `#@rust …` / `#@py …` for one engine only (used where the Rust
engine's stricter dead-branch reasoning differs from the prototype, e.g. `b15`).  A `#@`
directive must be the last thing on its line: the key runs to the end of the line.

Env: `ROWAN_DEBUG=1` (entry-point progress, slow instantiations), `ROWAN_COMPAT=1`,
`ROWAN_NO=recency,scc,site,narrowed,positional` (switch improvements off one by one).

## Port decisions (change, don't transcribe — and where it was transcribed anyway)

* Ids everywhere: keys, sources, lenses and site strings are interned; rows hold sorted id
  vectors, so join/compare are merges; `Row` is an `Arc` with a cached hash. Object ids are
  indices into `Analyzer.objects` (index 0 is a dummy so ids match the prototype's 1-based
  numbering in labels like `dict@L3#5`).
* The layered maps are a faithful port (the AGENTS.md plan mentioned `im`/`rpds`; the
  prototype's O(diff) join/diff via common-ancestor layers has no off-the-shelf equivalent
  and matters for footprints). One subtlety found by the differential test: an inner map
  seen on both sides of a join is only "inherited" if the ancestor layer holds that same
  `Arc` (`copy_ref` can put one map in two layers above the ancestor) — see `join_with`.
* AST nodes are `&'static` (parsed modules are leaked for the process; a library user
  analysing many programs in one process should expect that). The registry stores both the
  `Stmt`/`Expr` enum address and the payload struct address — the interpreter uses either.
* Python's `Any` payloads became `Payload` enum variants; `("@", ph)` slot keys are tuple
  keys (`access::param_slot_key`); `"*"` is `KeyId(0)`.
* Ordering: wherever the prototype iterates a `set` or picks `next(iter(set))`, the port
  uses sorted-by-repr or smallest-id order. The prototype itself was deterministic on
  everything measured, and the differential check is exact, so this did not matter.
* `exits` is an `Arc<State>`; joining it with itself is a no-op (the prototype shares the
  same object).

## Options (default on; `--compat` = all off = the prototype)

* `site_resolution` (next step 3 of the old notes): at instantiation a placeholder's targets
  are grouped by (kind, allocation site, entry rows); one exact concretisation per group
  (`summary::Inst::focus_groups`), instead of per target up to 8 and a joined row per site
  beyond. No verdict changes on anything measured; `models.py` 154 s → 99 s.
* `recency`: an allocation inside a loop (frame `single_nl`, i.e. single ignoring loops)
  returns a strong *recent* object (`site#recent`); at the loop back-edge `rename_recent`
  folds its rows/slots/elems into the weak per-site summary object and re-points every
  reference written during the iteration (footprint vs the loop head, env, narrowed, ret,
  overlays, payloads of objects allocated in the iteration, class attributes). Module-level
  name bindings inside loops are strong under this option (weak in the prototype). Break and
  return paths keep the recent object (correct: on those paths it is the current one).
  Effect: `f37_loop_recency.py`; `models.py` 99 s → 41 s, `interp.py` 434 s → 69 s; no access
  newly ✗, but accesses are reported under more call contexts (counts rise).
* `scc`: a summary computed while it used the *partial* summary of a function further down
  the summary stack is provisional (`summary_provisional`, with its dependencies); it is
  dropped when that function starts a new round or finalises, so callees are re-summarised
  against each round. This weakens some results that the prototype got by using a stale
  partial summary (`summary.py`: ⚠ 166 → 294, mostly the same accesses in more contexts, a
  handful of ✓~/◌ → ⚠ on `State.copy`/`join`-shaped recursion). Sounder, noisier; costs
  ~35 % on `models.py` (41 s with, 30 s without; verdicts unchanged there).
* `narrowed_same`: `State::same` compares `narrowed`. No measured effect.
* `positional`: unpacking a literal tuple key (`a, b = key` where `key` is `Lit((x, y))`
  or a `Lits` of same-arity tuples) yields per-position key values instead of data. The
  old note "container element positions are weak" was not reproduced on a concrete example;
  this is the one positional gap that was found.
* `fixes` (with the separately switchable `ctx_col` and `early_base`): prototype gaps found
  by the `g*` cases, all confirmed identical in the prototype first —
  summaries: the entry snapshot is taken *before* parameters are bound, so cells holding
  captured parameters and the `*args`/`**kwargs` containers are instantiated per call
  (closures over parameters, `compose`, decorators with arguments); a data cell read inside
  a nested summary becomes an entry fact instead of ⊥; call contexts are keyed by line *and*
  column (two calls on one line no longer share clones) and stacked decorators are applied
  at the decorator's own site; clones of distinct templates that land on one object (levels
  of a recursive structure) combine instead of overwriting; a parameter-keyed store whose
  key is unknown opens the row as a map instead of being dropped, and a parameter-keyed load
  on a closed record the function built itself adds no placeholder; requirements recorded
  under an `if`/`try`/boolean operator (not a loop) are "sometimes"; `in`/`not in`, `del`
  and `isinstance`-style narrowing on a parameter ask for per-call analysis when the call
  site has a literal / concrete value —
  flow: literal loops containing `break`/`continue` are unrolled; `if` prunes a branch when
  the test is definitely truthy/falsy; `k in d` on a closed record without `k` makes the
  branch infeasible (so `__contains__`-based guards and `assert "k" in d` can kill code);
  a guarded load in a real `try` narrows the key in on the fall-through path; after a loop
  without `break`, the iterable's element objects keep the body's exit row (every element
  went through the body) —
  values: bytes/Ellipsis constants are keys (port bug); slicing a list yields a list;
  `sorted`/`reversed` call the `key` callback and drop positions; `yield from` yields the
  iterable's elements; `functools.partial` objects are callable; `Cls.classmethod(...)`
  binds the class; dataclass fields include inherited ones in order; instances of different
  classes constructed at one site are distinct objects; `dict.get(k, default)` omits the
  default when `k` is proven; `"%d" % i` formats; declared fields annotated with a user
  class hold an instance of it.
  `--compat` turns all of this off; `tests/diff_engines.py` still reports SAME everywhere.

Known prototype behaviours the new cases document rather than change: a successful access
narrows the key in (`r["x"]` after a possibly-present store makes later reads proven, a01);
`.get`/`.setdefault` record no requirement; comparisons are not evaluated (`d["a"] < 0`
keeps both branches); class attributes are not records.

## Measured (release build, Apple M-series; "✗" = definitely-missing lines in the report)

| input | prototype | rowan-rs `--compat` | rowan-rs default |
| --- | --- | --- | --- |
| golden suite (36/37 cases) | 0.3 s | 0.4 s (36 processes) | 0.4 s |
| `scopes/layered/domain/report/state.py` | 0.2–1.3 s, 0 ✗ | 0.00–0.02 s, 0 ✗ | 0.00–0.02 s, 0 ✗ |
| `summary.py` | 29 s, 0 ✗, 166 ⚠ | 0.6 s, 0 ✗, 166 ⚠ | 0.8 s, 0 ✗, 294 ⚠ |
| `models.py` | > 50 min | 154 s, 126 ✗ | 41 s, 165 ✗ |
| `interp.py` | > 50 min | 540 s, 1039 ✗ | 69 s, 961 ✗ |
| 137 scripts from local projects | — | — | 0.8 s total, no failures |

Differential: `tests/diff_engines.py` reports SAME on all 38 programs (verdict sets and
schema tables); `summary.py` alone is 737 verdicts / 116 schema entries. The ✗ on
`models.py`/`interp.py` are the prototype's false positives (e.g. `self.table[prefix + "*"]`
in `models.py`: stores through the `register` closure's cell are not seen by the
summary of `lookup`), now reachable because the runs finish.

## Known problems

* **PyO3 wheel does not load on this machine.** `maturin build --release` in `rowan-rs/py`
  produces a wheel, but `import rowan_rs` fails with dyld "mis-aligned LINKEDIT string
  pool" on macOS 26 / Xcode 26 (Apple clang 21) with rustc 1.91, also with `-ld_classic`
  and after `strip`. The CLI binary is unaffected. The harness therefore shells out to the
  binary; `run_golden.py` and `diff_engines.py` do not use the binding. Try a newer rustc or
  Linux before touching the binding code.
* `models.py` / `interp.py` self-analysis: fast now but noisy (see table); the ✗ there are
  the precision problems listed in SPECIFICATION.md §16 plus closure-cell stores not seen
  by summaries. Real-repository validation only measured robustness (no panics, no
  timeouts) and tier counts, not precision — the §15.4 runtime oracle is still to do.
* Recency: references held *only* in objects allocated before the iteration are not
  re-pointed (by construction they cannot exist, since the recent object is new), but
  flow-insensitive side tables other than `cls_slots` (`entry_instances`, `rec_table`
  states, `call_cache` entries) are not renamed; the cache keys include the heap so stale
  entries do not match, they just waste memory.
* Mutual recursion still iterates at most `MAX_SUMMARY_ROUNDS = 3` rounds with
  join-widening; with `scc` on, each round re-summarises the dependent callees, so the cost
  is rounds × SCC size.
* `Row::concretise` pre-resolves the referenced entry rows transitively before substituting
  (the prototype resolved lazily through callbacks); same results, but a row whose entry rows
  reference many placeholders resolves all of them even when a lens kills everything.
* Leaked ASTs (see above); a long-running server should re-exec or accept the leak.

## Next steps

1. Precision on self-analysis: the `register`-closure pattern (stores through a cell of a
   sibling closure seen only via the summary of the factory) and dispatch over `self.objects`
   values are the two dominant ✗/⚠ families in `models.py`/`interp.py`. Both are visible in
   `rowan-rs/target/release/rowan-rs rowan/models.py --no-rows --no-summaries`.
2. Spec §15.4 runtime oracle (`sys.settrace` + wrapped `__getitem__`) and a noise budget on
   real repositories; `tests/diff_engines.py` is the static half.
3. Report volume: verdicts are per call chain; collapse chains per site in the text report
   before tuning tiers (counts in the table above are inflated by contexts).
4. Binding: retry the wheel on a different toolchain; when it loads, `run_golden.py` can
   import `rowan_rs` and skip the subprocess.
5. Incremental analysis (summaries cached by body hash) and the LSP mode of §15.5.
