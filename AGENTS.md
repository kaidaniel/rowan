# AGENTS.md: working on rowan

`README.md` says what rowan does for a user. This file is for working on the engine: how it
works, the owner's conventions, the tests, the debugging switches, and where things are. It
must be true at the last commit. The details live in the code's doc comments.

## How rowan works

rowan is a whole-program **abstract interpreter** for Python (Rust, `rowan/`). It parses with
ruff (private names mangled as CPython does) and walks the syntax tree, running the program
on abstract values. Imports run as Python runs them (`TYPE_CHECKING` is False, `import *`,
namespace packages).

* **Values** (`Value`, `domain.rs`) are a set of object ids plus a scalar part (`KeyVal`): a
  literal, up to 64 literals, the keys of some objects, or `Data(label)`, an unknown named by
  where it comes from (`row["id"]`, `x.shape[0]`). A label can also hold a tensor shape.
* **Objects** (`Obj`, `interp/heap.rs`) are identified by allocation site and context (the call
  chain, 2 deep). A *single* object stands for one runtime object: stores into it are strong.
  Allocating a site again while its earlier object is live gives a variant (up to 8), then a
  *merged* object (stores only join). Garbage collection at returns, loop exits and try exits
  drops the state of objects made since (`Obj::born`, renewed by a strong re-allocation) that
  nothing reaches.
* **State** (`State`, `state.rs`) is one point on one path: names → values, and per object
  its **rows** (`(object, namespace)` → `Row`), slots (the value under each key), elements,
  lengths, and the requirements recorded so far. The maps are persistent Patricia tries
  (`layered.rs`): a copy is O(1), and a join or diff costs what two states differ in.
* **Rows** are the key-set model: `present` (may be there) ⊇ `likely` (there unless unknown
  code removed it) ⊇ `definite` (there on every path), plus `absent`, open `sources` (inputs),
  and `mutators` (unknown calls). A row without sources or mutators is *closed*. A successful
  read narrows the key in.
* **Statements.** A test is evaluated once into the states where it is true and false
  (`split`, `narrow.rs`: narrowing reads its parts' values, `traced`); both branches run, join.
  Loops over at most 16 known items are unrolled (comprehensions calling nothing per item: 64);
  others run to a fixpoint (40 rounds, then widened; verdicts are the last round's). A running `try`
  (`TryCtx`) records the state at every point of its body and callees that may raise (a raise,
  a failing read, a model's raise, any unknown call: class `any_exc`), joined per class;
  handlers start from the raises they may catch, the rest go on through the `finally` to the
  enclosing try. `with` is a `try` whose `finally` is `__exit__`. `finally` runs on every way
  out, `break` / `continue` / `return` included.
* **Calls** (`calls.rs`): every call of user code runs the callee's body at the call site, in
  a context of the innermost `chain` (2) call sites. **Budget:** each function code (all
  closures of one `def`) may run 4,096 statements in all (`budget`); past that its calls are
  unknown calls that may also change what its code reaches beyond its arguments (`effects.rs`:
  the globals and cells its code, and the code it reaches by name, reads where they may change
  or escape; class attributes by name; mutable defaults; a method's receiver only by the names
  its code uses on it unless it escapes, `receiver_effects`), and the function gets a reporting
  run with open parameters (its lines then hold for the calls that did not run). Entry runs
  are not counted (one per function). So the work is bounded by functions times the budget.
  * **Recursion:** the inner call uses the running activation's partial result; the activation
    reruns from the heaps of its recursive calls too (`rec_heap`), 8 rounds, then widened.
  * **Unknown values** are not taken for instances of the program's classes: a method of an
    unknown object is an unknown call (no duck typing; it was a closed-world assumption).
  * **Unknown calls** may change everything they reach through what they are given, at any
    depth: records get a mutator, slots may hold anything (one shared unknown object, which
    no store changes: a value stored into it goes to unknown code, `escape`), lists and sets
    any items and length; the program's functions among it may run (`follow_code`). A walk
    by the same code from the same values walks only what changed since (`havoc_memo`).
    They return an unknown and may raise anything. Library functions known to have no effect
    are `noeffect` (`ALLOWLIST_PREFIXES`), unless they change their arguments (`random.shuffle`)
    or are given the program's functions. Several callees run as `alternatives`.
  * **Builtins** run the dunders Python calls for them (`builtin_dunder`); one without a model
    is unknown code. A TypeError may come from those taken to fail only on wrong types.
  * **Entry points:** functions nothing calls run with open parameters; their requirements
    become `param F.x` schemas.
* **Generators** run whole at their first consumption (`force_gen`), in the creating call's
  context; a consumer of item *k* sees the effects up to yield *k* (`GenEffects`). If code
  between items changes what the generator reads after a yield (`gen_watch`), it runs again
  with that changed at each yield (`with_gen_restart`). `@contextmanager` bodies run at yields.
* **Requirements and schemas:** reading a key from a row open through an input records a
  requirement on that input. A key read on every path is `always`, on some paths `sometimes`.
  A read guarded by a handler for its `KeyError` still counts (a frame column read behind a
  `try`, here or in a caller, is a probe: it does not).
* **Verdicts:** every read, `del` and `pop` gets a tier per object from its row (`classify`):
  `proven`, `possibly`, `missing` and `input` are sound; `likely` / `likely-missing` are
  optimistic (they assume unknown code removed nothing).
* **Models** (`models/`, by qualified name in `Registry`) compute CPython's result on known
  values (str / bytes, formatting, json, re, datetime, decimal, sqlite, csv…), and simulate
  pandas columns, pydantic / FastAPI validation and routing, and torch / numpy shapes (dims as
  polynomials; an undecided condition is a requirement and then a fact on the path; in torch
  modules `int` / `Tensor` annotations hold; a module-level unknown number names its dims).
* **Deadline:** a safety net only (`ms`, 10 s): past it user calls become unknown calls and
  running calls stop where they are. The budget, not the deadline, bounds the work.
* **Soundness** holds relative to the models and to annotations of values whose class is
  unknown. `exec`, `setattr` / `getattr` with unknown names (also in code a skipped call
  reaches), metaclasses and monkey-patching (rebinding a function or class too) are unknown or
  ignored. Accepted imprecision: lengths past 15, slices by unknown bounds, weak stores through
  names holding several objects, runs cut by a deadline; a generator does not see what unknown
  code changes between its items. An object a path has no facts of reads as unknown (`read_row`).

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (`.python-version` pins 3.14). The
  Python side is only the test harnesses.
* **New test cases are specifications.** Write the expected rows and verdicts by hand from
  the program text first, then make the engine produce them. Never derive an expectation from
  engine output. When an expectation turns out wrong, fix it with a comment saying why,
  keeping the line count. Value analysis matters more than bug-finding verdicts.
* **Hypotheses need a repro.** For a suspected bug or slowdown, write a small program showing
  it first (a golden case; `#@work` / `#@tabulations` bounds for cost), fix it, then measure.
* **Soundness first.** Where the runtime oracle and this file disagree, the oracle wins. Golden
  cases matter; snapshot changes are fine when sound.
* **Commits.** Commit when a round of work is complete and golden, snapshots and `cargo test`
  pass; never commit a failing state. Review snapshot diffs before `--update`.
* **Plans.** When a request mixes several things, write the plan first, then work it in order.
* **Output is for Python engineers.** The text report, hovers and diagnostics use words and
  Python access paths (`row["id"]`, `items[*].sku`), never engine terms (row, tier, source,
  placeholder) or internal site labels (`report::site_text` names unknown code by the call as
  written); every mark has a legend entry. `vscode-rowan/test.js` checks this on real hovers.
* `SUGGESTED_CHANGES.md` is the owner's list of open items, untracked: never commit it; keep it
  to things still to do. Keep this file under 200 lines and up to date with every change.

## Commands

```
cd rowan && cargo build --release && cargo test --release --lib && cd ..
uv run --no-project python tests/run_golden.py      # all suites, ~5 s; paths select cases; -v
uv run --no-project python tests/run_golden.py --pending [--prune]   # cases in tests/*/PENDING
uv run --no-project python tests/run_snapshots.py   # --update only after reviewing the diff
uv run --no-project --with pandas --with pydantic --with pydantic-settings --with fastapi --with httpx --with numpy \
    python tests/oracle.py [--strict|--lenient] [-v] [-j N] [case…]  # vs real runs, ~1.5 min
uv run --no-project --with torch --with numpy python tests/torch_oracle.py [case…]
cd vscode-rowan && node test.js && npm run package  # hovers; the .vsix (test_package.js X.vsix TARGET)
uv run --no-project python bench/run.py rich rowan/target/release/rowan "$(uv run --no-project python bench/prepare.py rich)"  # also email, stdlib12k, click; pip, timm, sktime, functional
```

Run the oracle on the cases you touched, and in full before a commit that changes models. CI
(`ci.yml`) runs the above but torch and the benchmark (`bench.yml`, also the `budget=0` golden
count), and tests a packed .vsix per native target (linux-x64, darwin-arm64, win32-x64).

## Tests

* **Golden suites** `tests/{cases,torch,pandas,pydantic,fastapi,stdlib,apps,numpy}/`, about a
  thousand programs; library suites have `small/` and `models/` (torch also `playground/`, the
  owner's programs); each has a `PENDING` list.
* **Directives** (full rules in `tests/CONVENTIONS.md`, also `tests/{torch,pandas}/`):
  `#@ tier key`, `#@row NAME: present= definite= absent= closed|open`,
  `#@value NAME: exactly= | unknown | includes=`, `#@schema src: always= sometimes= only=`,
  `#@todo …`, `#@argv …`, `#@shape` (torch). Cost bounds for blow-up cases, N by hand:
  `#@objects`, `#@tabulations`, `#@joins`, `#@work`, `#@visits`, `#@havoc <= N`; `#@limits
  k=v,…` sets the case's `ROWAN_LIMITS`. A directive-only line applies to the code line above.
* **Oracle** (`tests/oracle.py`): reruns each case under CPython with a tracer and checks every
  directive; schemas by dropping keys from the input. Strict on the library suites.
* **Snapshots** (`tests/snapshots/`): `--json` on `examples/etl_report.py` and
  `tests/fixtures/proto_*.py`. The regression guard for internals; object ids may shift.
* **Unit tests** in `rowan/src`: CPython-exact value functions, the regex engine, the tries.

## Debugging switches

* `ROWAN_DEBUG=1`: body runs (`run F at SITE`), skipped calls (`skip F …`), entry runs, a `work:` line
  (statements, units = statements + walk visits / 16, body runs, objects, row joins, visits,
  havoc visits): deterministic counters (its lines cost ~25%: time without). `2`: raises, tries.
* `ROWAN_LIMITS=budget=4096,ms=10000`: the only knobs (the golden `#@limits` sets them per
  case; `ROWAN_FOLLOW=pkg,…` is `--follow`; `ROWAN_TODO=check` checks `#@todo`s too).
* **Profiling:** the bench workflow's `perf` profiles (`sample` hangs on macOS 27). macOS has
  no `timeout`; time a run inside the guard (`.guard/run /usr/bin/time -l rowan …`: the guard
  polls every 0.5 s). Compare builds on results (`--json` tier counts) and counters too.

## Layout (`rowan/src/`, ~62k lines: the engine ~20.7k, the models ~42k)

| file | role |
| --- | --- |
| `intern.rs`, `domain.rs` | interned keys, sources, lenses; `Row`, `KeyVal`, `Value`, transfers, `classify` |
| `layered.rs`, `state.rs` | persistent maps (`LayerMap`, `TwoLevel`); `State`, joins, `diff_keys` |
| `frontend.rs`, `scopes.rs`, `interp/program.rs` | ruff parsing, leaked `&'static` trees, module registry; lexical scopes; `Program` (paths, models, modules imported), imports |
| `interp/mod.rs`; `entry.rs`; `heap.rs`, `alloc.rs`, `gc.rs` | `Analyzer`, side tables; running the program, entry runs (`Entries`); the `Heap` (objects, lengths, birth marks), allocation policy (`alloc`: variants, recency), liveness (`live_among`, `gc_pinned`) and collectors |
| `interp/stmts.rs`, `interp/tries.rs` | statements, loops, match; raises, try contexts (`Unwind`), try / with, `finally` |
| `interp/exprs.rs`, `interp/funcs.rs`, `interp/gens.rs` | expressions, comprehensions; functions, names, iteration; generators and `@contextmanager` bodies (`Gens`: lazy runs, per-yield states, watched runs) |
| `interp/narrow.rs`, `interp/access.rs` | tests (`split`), narrowing; attribute and item access, verdicts, requirements |
| `interp/calls.rs`, `recursion.rs`, `budget.rs`, `unknown.rs`, `effects.rs` | calls: per-call-site runs; recursion (`Recursion`: activations, fixpoint rounds); `Budget` (per code, deadline, work counters); `__new__`, dataclass init; unknown calls (`Havoc` walks); what skipped calls may change (`skipped_call`, `EffectsCache`) |
| `models/shape.rs` | tensor shapes (dims as polynomials) for the torch and numpy models |
| `interp/findings.rs`, `interp/names.rs`, `report.rs`, `main.rs` | `Findings` (verdicts, notes, row checks, hover facts) and `Names` (symbols, origins) collected; text / JSON / facts output; CLI (mimalloc), `rowan serve` (on a 1 GiB-stack thread) |
| `models/` | `mod.rs` (`Registry`, builtins, containers); `tables` (what builtins / library calls raise, change, keep); CPython-exact values (`pyvalues`, `pytext`, `pynum`, `chrono`, `pathtext`); `re` (`pyre`, `remodel`); stdlib (`enums`, `exceptions`, functools, itertools, …); `pandas`, `pydantic`, `fastapi`, `torch` |

`rowan/py/` is a PyO3 binding (does not load on macOS 26); `vscode-rowan/` the editor
extension (talks to `rowan serve`).

## Implementation rules that are easy to break

* **Models allocating several objects at one node** must give each its own context.
* **Callees** run with the caller's `state.exits` taken out and restored (y438).
* **GC roots.** A side table keyed by objects goes into `side_edges`; anything else the
  analyzer holds (a running with's managers) into `gc_pinned`: else a read gets an empty row.
* **Generator yields** use `alloc_raw` for their per-yield states, not `alloc`.
* **Stdlib objects** (`stdobj.<kind>`) keep attributes in Attr slots, dispatch via `m_stdobj`.
* **Attributes resolved by class** (`class_resolves`): class objects, model objects and
  `__getattr__` classes have more than their rows show.

## Measured (release build, Apple M-series)

| input | time |
| --- | --- |
| golden / oracle | ~5 s / ~1.5 min |
| stdlib12k (12k lines) / email (10k) / rich (27k) / click (10k), no deadlines | ~2.2 / ~2.6 / ~2 / ~7 s; 0.12 / 0.07 / 0.12 / 0.08 GB |
| pip (134k) / timm (132k) / sktime (355k) / functional (213k), no deadlines | ~8.3 / ~29 / ~64 / ~6.6 s; 0.26 / 0.9 / 1.0 / 0.28 GB |

## Known problems and next steps

Open items are in `SUGGESTED_CHANGES.md`, each with a repro; what was measured against the core
design (and ruled out) is there too. Parked: `parked/joined-summaries` (unsound), `slotmap-trie`.
