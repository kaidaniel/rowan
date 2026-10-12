# AGENTS.md: working on rowan

`README.md` says what rowan does for a user. This file is for working on the engine: how it
works, the owner's conventions, the tests, the debugging switches, and where things are. It
must be true at the last commit. The details live in the code's doc comments.

## How rowan works

rowan is a whole-program **abstract interpreter** for Python (Rust, `rowan/`). It parses with
ruff and walks the syntax tree directly, running the program on abstract values.

* **Values** (`Value`, `domain.rs`) are a set of object ids plus a scalar part (`KeyVal`): a
  literal, up to 64 literals, the keys of some objects, or `Data(label)`, an unknown named by
  where it comes from (`row["id"]`, `x.shape[0]`). A label can also hold a tensor shape.
* **Objects** (`Obj`, `interp/mod.rs`) are identified by allocation site and context (the call
  chain, 2 deep). A *single* object stands for one runtime object: stores into it are strong.
  Allocating a site again while its earlier object is live gives a variant (up to 8), then a
  *merged* object (stores only join). Garbage collection at returns drops dead objects' state.
* **State** (`State`, `state.rs`) is one point on one path: names → values, and per object
  its **rows** (`(object, namespace)` → `Row`), slots (the value under each key), elements,
  lengths, and the requirements recorded so far. The maps are persistent Patricia tries
  (`layered.rs`): a copy is O(1), and a join or diff costs what two states differ in.
* **Rows** are the key-set model: `present` (may be there) ⊇ `likely` (there unless unknown
  code removed it) ⊇ `definite` (there on every path), plus `absent`, open `sources` (inputs),
  and `mutators` (unknown calls). A row without sources or mutators is *closed*. A successful
  read narrows the key in.
* **Statements.** Both sides of a branch run, narrowed by the test (`narrow.rs`), and join.
  Loops over at most 16 known items are unrolled; other loops run to a fixpoint (40 rounds,
  then widened). `try` collects the state at each point that may raise; handlers start from
  those states. `with` is a `try` whose `finally` is `__exit__`. `finally` runs on every way
  out, `break` / `continue` / `return` included.
* **Calls** (`calls.rs`): every call of user code runs the callee's body at the call site,
  in a context of the innermost `chain` (2) call sites. The call cache replays a run when the
  same input recurs. **Budget:** each function code (all closures of one `def`) may run 4,096
  statements in all (`budget`); past that its calls are unknown calls. Entry runs are not
  counted (one per function). So the work is bounded by the number of functions times the
  budget, not by a deadline.
  * **Recursion:** the inner call uses the running activation's partial result; it iterates
    to a fixpoint (8 rounds), then widens.
  * **Unknown calls** add a mutator to every record they reach, return an unknown, and may
    raise anything.
  * **Entry points:** functions nothing calls run with open parameters; their requirements
    become `param F.x` schemas.
* **Generators** run eagerly at the call. A consumer of item *k* sees the effects up to yield
  *k* (`GenEffects`). A `@contextmanager`'s `with` body runs at each yield.
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
  polynomials; an undecided condition is a requirement and then a fact on the path).
* **Deadlines:** a safety net only. Past the soft one (`ms`, 2 s) allocation sites stop
  making new clones; past the hard one (5×) user calls become unknown calls and running calls
  stop where they are. With `work=N` both count work units instead (statements and walk
  visits at 16 per unit).
* **Soundness** holds relative to the models and to annotations of values whose class is
  unknown. `exec`, `setattr` / `getattr` with unknown names, metaclasses and monkey-patching
  are unknown or ignored. Accepted imprecision: lengths past 15, slices by unknown bounds,
  weak stores through names holding several objects, runs cut by a deadline.

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (`.python-version` pins 3.14). The
  Python side is only the test harnesses.
* **New test cases are specifications.** Write the expected rows and verdicts by hand from
  the program text first, then make the engine produce them. Never derive an expectation from
  engine output. When an expectation turns out wrong, fix it with a comment saying why,
  keeping the line count. Value analysis matters more than bug-finding verdicts.
* **Hypotheses need a repro.** For a suspected bug or slowdown, write a small program showing
  it first (it becomes a golden case, with `#@work` / `#@tabulations` (body runs) bounds for cost), fix
  it, then measure on the big programs.
* **Soundness first.** Where the runtime oracle and this file disagree, the oracle wins. Golden
  cases matter; snapshot changes are fine when sound.
* **Commits.** Commit when a round of work is complete and golden, snapshots and `cargo test`
  pass; never commit a failing state. Review snapshot diffs before `--update`.
* **Plans.** When a request mixes several things, write the plan first, then work it in order.
* `SUGGESTED_CHANGES.md` is the owner's list of open items. It is untracked: never commit it.
  Keep it to things still to do.
* Keep this file under 200 lines and up to date with every change.

## Commands

```
cd rowan && cargo build --release && cargo test --release --lib && cd ..
uv run --no-project python tests/run_golden.py      # all suites, ~5 s; paths select cases; -v
uv run --no-project python tests/run_golden.py --pending [--prune]   # cases in tests/*/PENDING
uv run --no-project python tests/run_snapshots.py   # --update only after reviewing the diff
uv run --no-project --with pandas --with pydantic --with pydantic-settings --with fastapi --with httpx --with numpy \
    python tests/oracle.py [--strict|--lenient] [-v] [-j N] [case…]  # vs real runs, ~1.5 min
uv run --no-project --with torch --with numpy python tests/torch_oracle.py [case…]
cd vscode-rowan && node test.js && npm run package  # hover rendering check; builds the .vsix
uv run --no-project python bench/run.py rich rowan/target/release/rowan "$(uv run --no-project python bench/prepare.py rich)"
                                                    # benchmark (stdlib12k / email / rich), no deadlines
```

Run the oracle on the cases you touched, and in full before a commit that changes models. CI
(`.github/workflows/ci.yml`) runs all of the above except torch and the benchmark (`bench.yml`).

## Tests

* **Golden suites** `tests/{cases,torch,pandas,pydantic,fastapi,stdlib,apps,numpy}/`, about a
  thousand programs; library suites have `small/` and `models/`; each has a `PENDING` list.
* **Directives** (full rules in `tests/CONVENTIONS.md`, also `tests/{torch,pandas}/`):
  `#@ tier key`, `#@row NAME: present= definite= absent= closed|open`,
  `#@value NAME: exactly= | unknown | includes=`, `#@schema src: always= sometimes= only=`,
  `#@todo …`, `#@argv …`, `#@shape` (torch). Cost bounds for blow-up cases, N by hand:
  `#@objects`, `#@tabulations`, `#@joins`, `#@work`, `#@visits <= N`; `#@limits k=v,…` sets the
  case's `ROWAN_LIMITS`. A directive-only line applies to the code line above.
* **Oracle** (`tests/oracle.py`): reruns each case under CPython with a tracer and checks every
  directive; schemas by dropping keys from the input. Strict on the library suites.
* **Snapshots** (`tests/snapshots/`): `--json` on `examples/etl_report.py` and
  `tests/fixtures/proto_*.py`. The regression guard for internals; object ids may shift.
* **Unit tests** in `rowan/src`: CPython-exact value functions, the regex engine, the tries.

## Debugging switches

* `ROWAN_DEBUG=1`: every body run (`run F at SITE`), entry runs, and a final `work:` line
  (units, body runs, objects, row joins, visits). `2`: also every raise and try decision. With it, `ROWAN_GCDBG=1` prints what each collection drops.
* `ROWAN_LIMITS=ms=2000,hard=10000,budget=4096,clones=256,chain=2,path=3`. `work=550000`
  (about 2 s) gives reproducible runs for comparisons. A change that removes liveness walks
  spends a `work=N` budget on more statements: compare progress in equal time
  (`ms=60000,hard=60000`) too.
* `ROWAN_GC=0`: almost no collection.
  `ROWAN_FOLLOW=pkg,…` = `--follow`. `ROWAN_TODO=check` checks `#@todo`s too.
* **Profiling:** short runs, several in parallel. `sample <pid> 3` for time,
  `MallocStackLogging=lite` + `malloc_history <pid> -allBySize` for memory. macOS has no
  `timeout`: `perl -e 'alarm N; exec @ARGV' rowan …`. Compare builds on results (proven /
  missing / possibly / schema counts of `--json`), not only on counters.

## Layout (`rowan/src/`, ~64k lines)

| file | role |
| --- | --- |
| `intern.rs`, `domain.rs` | interned keys, sources, lenses; `Row`, `KeyVal`, `Value`, transfers, `classify` |
| `layered.rs`, `state.rs` | persistent maps (`LayerMap`, `TwoLevel`); `State`, joins, `diff_keys` |
| `frontend.rs`, `scopes.rs` | ruff parsing, leaked `&'static` trees, module registry; lexical scopes |
| `interp/mod.rs` | `Analyzer`, side tables, `Obj`, allocation (`alloc_full`, `live_among`), lengths, GC, entry runs |
| `interp/stmts.rs` | statements, loops, try / with / match, `finally` |
| `interp/exprs.rs`, `interp/funcs.rs` | expressions, comprehensions, yields; functions, iteration, generators |
| `interp/narrow.rs`, `interp/access.rs` | narrowing, `CondFact`s; attribute and item access, verdicts, requirements |
| `interp/calls.rs` | calls: per-call-site runs, budget, call cache, recursion, `__new__`, dataclass init |
| `shape.rs` | tensor shapes |
| `report.rs`, `main.rs` | text / JSON / facts output; CLI, `rowan serve` (runs on a 1 GiB-stack thread) |
| `models/` | `mod.rs` (`Registry`, builtins, containers); CPython-exact values (`pyvalues`, `pytext`, `pynum`, `chrono`, `pathtext`); `re` (`pyre`, `remodel`); stdlib; `pandas`, `pydantic`, `fastapi`, `torch` |

`rowan/py/` is a PyO3 binding (does not load on macOS 26); `vscode-rowan/` the editor
extension (talks to `rowan serve`).

## Implementation rules that are easy to break

* **Models allocating several objects at one node** must give each its own context, or the
  second overwrites the first.
* **Narrowing never re-runs a call:** each call node's last value is read back (`call_memo`).
* **Callees** run with the caller's `state.exits` taken out and restored (y438).
* **GC roots.** Anything a side table refers to must be in `gc_pinned` or reachable through
  `obj_edges`; otherwise a later read gives a silently empty row.
* **Generator yields** use `alloc_raw` for their per-yield states, not `alloc`.
* **Stdlib objects** (`stdobj.<kind>`) keep attributes in Attr slots and dispatch through
  `m_stdobj`.
* **Attributes resolved by class** (`class_resolves`): class objects, model objects and
  `__getattr__` classes have more than their rows show.

## Measured (release build, Apple M-series)

| input | time |
| --- | --- |
| golden / oracle | ~5 s / ~1.5 min |
| stdlib12k (12k lines) / email (10k) / rich (38k), no deadlines | ~8 / ~6 / ~8 s; 0.1 / 0.9 / 0.6 GB |

## Known problems and next steps

Open bugs, performance items and precision to-dos are in `SUGGESTED_CHANGES.md`, each with a
repro; work it in order. The main ones: the interpreter is still far from its 10k-line goal,
precision lost where a function's budget runs out on large programs, and the precision to-dos
(`#@todo`s in the suites). Symbolic summaries were removed (they were the main source of
unsoundness and cost; the parked `parked/joined-summaries` branch tried to repair them). The branch `slotmap-trie` is parked (same
answers, no gain).
