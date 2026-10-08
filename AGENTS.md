# AGENTS.md — rowan, handoff notes

Static key-set inference for Python records: infers, for every dict-like value (dicts,
DataFrame columns, object attributes, kwargs, JSON/CSV rows, argparse namespaces…), which
keys it has at each point and which keys the program demands of it; demands on external
inputs become inferred schemas. Design: `SPECIFICATION.md` (17 sections, Appendix A golden
cases). User-facing summary: `README.md`. This file is for whoever picks the work up.

## State of the code

One engine, in Rust (`rowan/`, ~14k lines). It began as a port of a Python prototype
(frozen as the oracle while porting, differential-tested to identical verdict sets, then
removed in the commit after `a1f4d7a`); since then it has grown algorithmic improvements and
semantic fixes that the prototype did not have, so the prototype is history, not a reference.

| file | role |
| --- | --- |
| `rowan/src/intern.rs` | thread-local interners: keys (`Key` → `KeyId`), strings, sources, lenses; `IdSet<T>` = sorted `Arc<[T]>` with merge-based set ops (`KeySet`, `ObjSet`, …). |
| `rowan/src/domain.rs` | lens steps, `Source`, `Row` (`Arc` + cached hash, all sets as `IdSet`s), `classify`, `KeyVal`, `ReqInfo`, `Value`, `chain_extend`. |
| `rowan/src/layered.rs` | `LayerMap` / `TwoLevel`: frozen layers with a private top, O(1) copy, O(diff) join/same/diff_keys, `copy_ref` (copy from a shared reference, used for `exits`); randomised tests against a naive model. |
| `rowan/src/state.rs` | `Kind`, `State` (env, narrowed, rows, slots, elems, reqs, ret, exits as `Arc<State>`, symbolic), `Footprint`. |
| `rowan/src/scopes.rs` | lexical scopes (locals / cells / free / globals) over the ruff AST. |
| `rowan/src/frontend.rs` | parsing (`ruff_python_parser`), leaked `&'static` module ASTs, a thread-local registry mapping any node address to its module so a node carried across modules (a deferred access in a summary) still knows its file and line; `Node` = located node (synthetic nodes for internal fake calls). |
| `rowan/src/interp/mod.rs` | `Analyzer` struct, `Options`, `Obj`/`Payload`, `FuncInfo`/`ClassInfo` (`Rc`, `Cell` for the mutable flags), allocation (`alloc`, `alloc_raw`, `alloc_in_frame`), verdicts, entry points, `rename_recent`, `record_fact`. |
| `rowan/src/interp/{stmts,exprs,funcs,narrow,access,calls}.rs` | statements, expressions and comprehensions, functions / names / targets / iteration, narrowing, record access and attributes, calls and bindings. |
| `rowan/src/summary.rs` | placeholders, `SymReq`/events, `summarise` (rounds, partial and provisional summaries), `Inst` (instantiation with its caches), `focus_groups` (allocation-site resolution). |
| `rowan/src/models/{mod,pandas}.rs` | library models; `Registry::lookup` with prefix / allowlist fallbacks. |
| `rowan/src/report.rs` | dedupe, schema table, text, JSON lines, `--golden` and `--facts` documents. |
| `rowan/src/main.rs` | CLI; runs the analysis on a thread with a 1 GiB stack. |
| `rowan/py/` | PyO3 binding (`rowan.analyse(source, …)` / `analyse_file(path)` → `--golden` JSON string). Builds with maturin; see known problems. |
| `tests/run_golden.py`, `tests/cases/` | golden harness, 63 cases: `a01`–`e36` from the original design (Appendix A and the prototype era), `f37` (recency), `g40`–`g65` (AST node coverage, closures taking closures returning lambdas, reassignment, control flow, inputs indexing results of input functions, classes, string keys, containers, guards, modules, generators, recursion, kwargs forwarding, configs, pandas, annotations, mutation through layers, comprehensions, dispatch, a CLI script, dict methods, a larger mixed program). Directives: `#@ tier key [; …]`, `#@row`, `#@schema`; a `#@` directive must be the last thing on its line. |
| `tests/run_snapshots.py`, `tests/snapshots/`, `tests/fixtures/` | `--json` output snapshots on `examples/etl_report.py` and three large fixtures (modules of the old prototype kept as analysis *inputs* only). The regression guard for refactors of the internals; `--update` after reviewing a diff. |
| `vscode-rowan/` | VS Code extension (plain JS): runs `rowan FILE --facts` on open/save, hover = facts for (line, name), verdicts as diagnostics; `node test.js` checks the rendering. |

Run (Python only for the harness, via `uv`; `.python-version` pins 3.14):

```
cd rowan && cargo build --release && cargo test --release --lib
rowan/target/release/rowan FILE.py [--json|--golden|--facts] [--no-rows --no-summaries --no-entry --no-imports]
uv run --no-project python tests/run_golden.py      # 63/63
uv run --no-project python tests/run_snapshots.py   # SAME on all snapshots
ROWAN_SUMMARIES=0 …                                 # old per-call-site engine (34/63; kept for comparison)
```

Env: `ROWAN_DEBUG=1` (entry-point progress, slow instantiations), `ROWAN_NO=recency,scc,site,narrowed,positional`.

## Design notes

* Ids everywhere: keys, sources, lenses and site strings are interned; rows hold sorted id
  vectors, so join/compare are merges; `Row` is an `Arc` with a cached hash. Object ids are
  indices into `Analyzer.objects` (index 0 is a dummy so ids are 1-based in labels like
  `dict@L3#5`).
* The layered maps keep the fork/join structure the design needs (O(diff) join and
  footprints via common-ancestor layers). One subtlety: an inner map seen on both sides of
  a join is only "inherited" if the ancestor layer holds that same `Arc` (`copy_ref` can put
  one map in two layers above the ancestor) — see `join_with`.
* AST nodes are `&'static` (parsed modules are leaked for the process; a library user
  analysing many programs in one process should expect that). The registry stores both the
  `Stmt`/`Expr` enum address and the payload struct address — the interpreter uses either.
* Where the old prototype iterated a `set`, the engine uses sorted-by-repr or smallest-id
  order; everything is deterministic.
* `exits` is an `Arc<State>`; joining it with itself is a no-op.
* Call contexts are keyed by call site line *and* column; instance allocation sites include
  the class; decorators are applied at the decorator's own site; clones of distinct
  templates that land on one object (levels of a recursive structure) combine their facts.
* A summary's entry snapshot is taken before parameters are bound, so cells holding captured
  parameters and the `*args`/`**kwargs` containers are instantiated per call; a data cell
  read inside a nested summary is an entry fact (path placeholder) of that summary.
* Requirements recorded under an `if`/`try`/boolean operator inside a summarised function
  are "sometimes" (`branch_depth`); inside a loop body they stay per-element (`always`).
* Flow: `if` prunes a branch when the test is definitely truthy/falsy; `k in d` on a closed
  record without `k` makes the branch infeasible (so `__contains__` guards and
  `assert "k" in d` can make code dead); a guarded load in a real `try` narrows the key in on
  the fall-through path; a successful access narrows the key in.

## Options (default on; `ROWAN_NO=…` switches one off)

* `site_resolution`: at instantiation a placeholder's targets are grouped by (kind,
  allocation site, entry rows); one exact concretisation per group (`Inst::focus_groups`).
* `recency`: an allocation inside a loop (frame `single_nl`, i.e. single ignoring loops)
  returns a strong *recent* object (`site#recent`); at the loop back-edge `rename_recent`
  folds its rows/slots/elems into the weak per-site summary object and re-points every
  reference written during the iteration (footprint vs the loop head, env, narrowed, ret,
  overlays, payloads of objects allocated in the iteration, class attributes). Module-level
  name bindings inside loops are strong under this option. Break and return paths keep the
  recent object. Golden case `f37`.
* `scc`: a summary computed while it used the *partial* summary of a function further down
  the summary stack is provisional (`summary_provisional`, with its dependencies); dropped
  when that function starts a new round or finalises, so callees are re-summarised against
  each round. Sounder and noisier than using the stale partial summary; ~35 % slower on
  large inputs.
* `narrowed_same`: `State::same` compares `narrowed`.
* `positional`: unpacking a literal tuple key yields per-position key values.

## Measured (release build, Apple M-series; "✗" = definitely-missing lines in the report)

| input | time | notes |
| --- | --- | --- |
| golden suite (63 cases) | 0.5 s | one process per case |
| `tests/fixtures/proto_summary.py` (1.2k lines) | 0.8 s | 0 ✗ |
| old prototype `models.py` (1.7k lines, reaches a whole interpreter via a typed field) | 41 s | 165 ✗ (false positives, see below) |
| old prototype `interp.py` (3.4k lines) | 69 s | 961 ✗ |
| 137 scripts from local projects | 0.8 s total | no failures; precision not assessed |

## Known problems

* **PyO3 wheel does not load on this machine.** `maturin build --release` in `rowan/py`
  produces a wheel, but importing it fails with dyld "mis-aligned LINKEDIT string pool" on
  macOS 26 / Xcode 26 (Apple clang 21) with rustc 1.91, also with `-ld_classic` and after
  `strip`. The CLI binary is unaffected; the harness and the extension shell out to it. Try
  a newer rustc or Linux before touching the binding code.
* Large self-referential programs (the old `models.py`/`interp.py`) are fast now but noisy:
  the ✗ there come from two families — stores through a sibling closure's cell seen only via
  the factory's summary (`register`-style tables), and dispatch over values of a `Dict[int,
  Obj]` whose contents the entry-point pass does not know. Real-repository validation only
  measured robustness (no panics, no timeouts) and tier counts, not precision — the
  spec §15.4 runtime oracle is still to do.
* Recency: flow-insensitive side tables other than `cls_slots` (`entry_instances`,
  `rec_table` states, `call_cache` entries) are not renamed; the cache keys include the heap
  so stale entries do not match, they just waste memory.
* Mutual recursion still iterates at most `MAX_SUMMARY_ROUNDS = 3` rounds with
  join-widening; with `scc` on, each round re-summarises the dependent callees.
* `Row::concretise` pre-resolves the referenced entry rows transitively before substituting;
  a row whose entry rows reference many placeholders resolves all of them even when a lens
  kills everything.
* Comparisons are not evaluated (`d["a"] < 0` keeps both branches); class attributes are not
  records; `match` patterns do not narrow (spec §16); no `getattr` with computed names;
  pandas ops with data-derived columns stay open.
* Leaked ASTs (see above); a long-running server should re-exec or accept the leak.

## Next steps

1. Precision on self-referential programs: the `register`-closure pattern (stores through a
   cell of a sibling closure seen only via the summary of the factory) and dispatch over
   `self.objects`-style tables are the two dominant ✗/⚠ families.
2. Spec §15.4 runtime oracle (`sys.settrace` + wrapped `__getitem__`) and a noise budget on
   real repositories; the snapshot tests are the static half.
3. Report volume: verdicts are per call chain; collapse chains per site in the text report
   before tuning tiers.
4. Binding: retry the wheel on a different toolchain; when it loads, the harness and the
   extension can import `rowan` and skip the subprocess.
5. Incremental analysis (summaries cached by body hash) and the LSP mode of §15.5 (the
   extension currently re-runs the CLI on save; a server would keep summaries warm).
