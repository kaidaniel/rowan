# AGENTS.md: notes for working on rowan

What rowan does and how to build it is in `README.md`, and how the engine works in
`SPECIFICATION.md`. This file holds what neither covers: the owner's conventions, the test
harnesses, debugging switches, where things are in the code, and what is open. It must be
true at the last commit.

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (`.python-version` pins 3.14), never
  the system or a local interpreter. The Python side is only the test harnesses.
* **New test cases are specifications.** Write the expected rows and verdicts by hand from
  the program text first, then make the engine produce them. Never derive an expectation
  from engine output. When an expectation turns out wrong on re-reading, fix it with a comment
  saying why, keeping the line count (schema directives name lines). Value analysis (which
  keys, which columns, which values) matters more than bug-finding verdicts.
* **Soundness first.** Where the runtime oracle and the spec disagree, the oracle wins; fix
  the spec.
* **Commits.** Commit without being asked when a round of fixes or a feature is complete and
  the tests pass (golden, snapshots, `cargo test`), one commit per round. Never commit a
  failing state. Review snapshot diffs before `--update`. Messages end with
  `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, naming the model at work.
* **Plans.** When a request mixes several things, write the plan first, then work it in order.
  If time remains, write example programs that probe specific hypotheses about bugs.
* `SUGGESTED_CHANGES.md` is the owner's working list of open items. It is untracked: never
  commit it. Keep it to things still to do.
* Keep this file under 200 lines and up to date with every change.

## Commands

```
cd rowan && cargo build --release && cargo test --release --lib && cd ..
uv run --no-project python tests/run_golden.py      # all suites, ~5 s; paths select cases; -v
uv run --no-project python tests/run_golden.py --pending [--prune]   # cases in tests/*/PENDING
uv run --no-project python tests/run_snapshots.py   # --update only after reviewing the diff
uv run --no-project --with pandas --with pydantic --with pydantic-settings --with fastapi --with httpx --with numpy \
    python tests/oracle.py [--strict|--lenient] [-v] [-j N] [case…]  # expectations vs real runs, ~1.5 min
uv run --no-project --with torch --with numpy python tests/torch_oracle.py [case…]
cd vscode-rowan && node test.js && npm run package  # hover rendering check; rebuilds rowan-0.1.0.vsix
cargo install --path rowan                          # the installed binary (~/.cargo/bin/rowan)
uv run --no-project python bench/run.py rich rowan/target/release/rowan "$(uv run --no-project python bench/prepare.py rich)"
                                                    # benchmark (stdlib12k / email / rich), no deadlines; CI: bench.yml
```

The oracle runs one fresh Python process per case, plus one per key a `#@schema` checks, each
under a line tracer, as many at a time as there are CPUs. Run it on the cases you touched while working,
and in full before a commit that changes models. CI (`.github/workflows/ci.yml`) runs the
build, `cargo test`, golden, pending (informational), snapshots and the hover test on Linux
and macOS, then the oracle on Linux. The torch oracle runs only on a manual dispatch with
`torch` set, and the `.vsix` is packaged on pushes to main.

## Test suites

* **Golden suites:** `tests/{cases,torch,pandas,pydantic,fastapi,stdlib,apps,numpy}/`, about a
  thousand programs.
  * Each library suite has `small/` (one API area per file) and `models/` (realistic
    programs). `apps/` holds small, medium and large package programs.
  * Each suite has a `PENDING` list, for cases written before the engine passes them.
* **Directives** (`tests/CONVENTIONS.md`; torch: `tests/torch/CONVENTIONS.md`; pandas:
  `tests/pandas/CONVENTIONS.md`):
  * `#@ tier key`;
  * `#@row NAME: present= definite= absent= closed|open`;
  * `#@value NAME: exactly= | unknown | includes=`;
  * `#@schema src: always= sometimes= only=`;
  * floors `possible=` / `definite_within=`; `#@todo …` for known imprecision; `#@argv …`;
    `#@objects <= N` (a bound on the objects made, for blow-up cases; N by hand);
  * torch: `#@shape`.

  A directive-only line applies to the code line above. `possibly k` also holds when rowan's
  per-key verdicts differ between runs (the oracle reads it per line).
* **Oracle** (`tests/oracle.py`): reruns each case under CPython with a tracer and checks
  every directive against the real values. Schemas are checked by dropping keys from the
  input. It is strict on the library suites.
* **Snapshots** (`tests/snapshots/`): `--json` output on `examples/etl_report.py` and three
  modules of the engine's original Python prototype (`tests/fixtures/proto_*.py`), with fixed
  caps and no deadline. This is the regression guard for internals. Object ids may shift;
  read the diff.
* `rowan/src` unit tests: the CPython-exact value functions, the regex engine, the
  Patricia-trie maps (model tests against `BTreeMap` with wide keys).

## Debugging switches

* `ROWAN_DEBUG=1|2`:
  * `1`: tabulation reasons, summaries, budgets, and a `work:` line at the end with counts;
  * `2`: also every event, raise and try decision.
* `ROWAN_NO=a,b` switches refinements off by name (`rg 'disabled\("' rowan/src` lists them).
  Use it to bisect which refinement causes a result.
* `ROWAN_LIMITS=ms=2000,hard=10000,tab=256,call=256,clones=256,chain=2,path=3`: the soft and
  hard deadlines, budgets, call-string depth and access-path depth. `work=550000` (about 2 s
  here) puts the deadlines on work units instead: reproducible runs of large programs for
  comparisons and profiles (the `work:` line of `ROWAN_DEBUG=1` shows the units).
* Experimental, off by default (measured not worth it, SUGGESTED_CHANGES 3.1):
  `ROWAN_LIMITS=genfull=1` (summaries for every generator). Summary variants per decided
  outcome were removed again (code in 96fa750).
* `ROWAN_SUMMARIES=0`: the old per-call-site engine, with no summaries.
* `ROWAN_GC=0`: garbage collection only for returns of values without objects.
  With `ROWAN_DEBUG=1`, `ROWAN_GCDBG=1` prints what each
  collection drops.
* `ROWAN_FOLLOW=pkg,…` is the same as `--follow`. `ROWAN_TODO=check` also checks `#@todo`s,
  to see which to-dos the engine now meets.
* **Profiling:** use short runs (a few seconds), several in parallel, long ones in the
  background. Use `sample <pid> 3` on the release binary (inclusive time per function from its
  call tree), and `MallocStackLogging=lite` + `malloc_history <pid> -allBySize` for memory
  (it slows the run: scale the deadlines). macOS has no `timeout`; use
  `perl -e 'alarm N; exec @ARGV' rowan …`. Compare builds on results (proven / missing /
  possibly / schema counts of `--json`), not only on counters, and with `work=N` when a
  deadline cuts the run. `bench/` (the other agent's harness: `bench/run.py`, programs in
  `bench/work/{stdlib12k,email,rich}`) runs without deadlines.

## Layout (Rust engine `rowan/src/`, ~64k lines)

| file | role |
| --- | --- |
| `intern.rs` | interners: keys (incl. `Member`, `Frozen`, `Dec` / `Frac`, `Chrono`), sources, lenses; `IdSet<T>` |
| `domain.rs` | `Row`, `KeyVal`, `Value`, `Lens`, `Source`, transfers, `classify` (tiers), requirements |
| `layered.rs` | persistent maps: `LayerMap` (Patricia trie, `PKey`, `Merge`), `TwoLevel` (copy-on-write inner hash maps) |
| `state.rs` | `State`, joins (`join_env`, rows, lens), `diff_keys` |
| `frontend.rs`, `scopes.rs` | ruff parsing, leaked `&'static` trees, node→module registry, parse cache, `site_prefix`; lexical scopes |
| `interp/mod.rs` | `Analyzer` and its side tables (enums, argparse, csv, futures, singledispatch, identity keys, cookie jars…), `Obj`, allocation (`alloc_full`, `alloc_raw`, variants, `live_among`), `llen` / `set_llen`, `copy_side_tables`, budgets, GC (`drop_garbage_at_return`, `obj_edges`, `gc_pinned`), entry runs |
| `interp/stmts.rs` | statements, loops (unroll, fixpoint, widening), try / with / match |
| `interp/exprs.rs` | expressions, comprehensions, yields |
| `interp/funcs.rs` | functions, iteration, generators (`GenEffects`, `YieldKey`) |
| `interp/narrow.rs` | narrowing, `CondFact`s, `flag_facts` |
| `interp/access.rs` | record and attribute access, verdicts, requirements |
| `interp/calls.rs` | calls: tabulation vs summary, call cache, dataclass init, singledispatch, exception objects, `strip_annotation` |
| `summary.rs` | placeholders, events, `summarise_sig`, `Inst` (instantiation), `propagate_events`, `should_tabulate` |
| `shape.rs` | tensor shapes: dims as polynomials, rests, broadcasting |
| `report.rs`, `main.rs` | outputs (text, JSON, facts, value rendering); CLI, `serve_loop`, site-packages discovery (runs on a 1 GiB-stack thread) |
| `models/mod.rs` | `Registry`; builtins, dict / list / set / str methods, `copy`, `dataclasses`, `collections`, `json`, `slice`, enum hooks |
| `models/{pyvalues,pure,pytext,pynum,chrono,pathtext}.rs` | CPython-exact values: str / bytes methods, formatting, math / operator / statistics, base64, shlex, html, urllib quoting; Decimal / Fraction; datetime; os.path / pathlib / textwrap |
| `models/{pyre,remodel,minire}.rs` | a Python-`re` backtracking engine and the `re` model; a small matcher for pandas `regex=` |
| `models/{counter,csvio,argparse,concurrency,itertools,stdfuncs,configparser,sqlite}.rs` | the rest of the stdlib (§11.1 of the spec); `stdobj.*` objects dispatch through `m_stdobj` |
| `models/{pandas,pydantic,fastapi,torch}.rs` | the library models |

`rowan/py/` is a PyO3 binding (does not load on macOS 26 / Xcode 26); `vscode-rowan/` the
editor extension (talks to `rowan serve`).

## Implementation rules that are easy to break

* **Models that allocate several objects at one node** in a Rust loop must give each object
  its own context (pydantic: per input record / container; `executor.map`, `object_hook`:
  column offsets). Otherwise the second allocation overwrites the first. A context passed to
  `alloc_at` is a suffix of the frame's, so the same code run from two callers makes two
  objects.
* **Narrowing never re-runs a call:** each call node's last value is read back (`call_memo`).
* **Module containers.** Module dicts, lists and sets that functions mutate are deferred loads
  in summaries (`deferred_global`), also when met through a callee's summary.
* **Tabulated callees and exits.** A tabulated callee runs with the caller's `state.exits`
  taken out and restored. Joining them made a caller's finished paths come back (y438).
* **GC roots.** Anything a side table refers to must be in `gc_pinned` or reachable through
  `obj_edges`. A collected object that is later read gives a silently empty row. Parts of an
  input are never collected.
* **Generator yields** use `alloc_raw` for their per-yield states (`gen_yield_states`); `alloc`
  adds a pid key that does not match.
* **Text labels decide events:** a known text label (`csvhdr:`, `pyjsontext:`) is a `concrete`
  event, so a reader's call site tabulates.
* **Stdlib objects** (`stdobj.<kind>`: StringIO, csv readers / writers, ExitStack, Template,
  Pattern / Match, paths, sqlite connections / cursors) keep attributes in Attr slots. A
  parameter whose attribute is named like one of a family the program uses decides per call
  site (`stdobj_attr_name`).
* Small generated modules stand in for namedtuple, itemgetter / attrgetter, UserDict and
  urllib.parse (ported from CPython).

## Measured (release build, Apple M-series)

| input | time | notes |
| --- | --- | --- |
| golden (all suites) | ~5 s | one process per case |
| oracle | ~1.5 min | cases in parallel (one thread per CPU) |
| `j93` (830 lines) / `m96_pkg` (1,716) | 0.09 / 0.10 s | |
| `proto_summary.py` (1.2k, self-referential) | 2.1 s (`--facts` 3.2 s, 13.6 MB) | |
| email (stdlib, 10k lines) / rich (38k) | ~10 / ~11 s | cut by the hard deadline; rich 1.1 GB |
| stdlib12k without deadlines | ~20 s | `bench/`; rich does not finish without deadlines (3.0) |

Large programs are not yet run to completion, and precision on real repositories has not
been measured.

## Known problems

The soundness stance and accepted imprecision are in SPECIFICATION §13. The bugs, performance
items and precision to-dos are listed, each with how to reproduce it,
in `SUGGESTED_CHANGES.md`. The main ones:

* runs cut by the clock deadlines depend on machine load (`ROWAN_LIMITS=work=N` does not);
* the `exits` fix doubled the objects on rich.

Other limits:

* the PyO3 wheel fails to load on macOS 26;
* the branch `slotmap-trie` (inner slot maps as tries) is parked: same answers, no gain.

## Where to go next

Work `SUGGESTED_CHANGES.md` in order:
the performance items (the rich object doubling, the `summarise_sig` tables, a work-based
budget), the on-demand server experiment, and the precision to-dos (`#@todo`s in the suites).
