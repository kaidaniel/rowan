# AGENTS.md: working on rowan

`README.md` says what rowan does for a user; this file is for working on the engine (how it works, the
owner's conventions, tests, switches, layout), true at the last commit; details are in doc comments.

## How rowan works

rowan is a whole-program **abstract interpreter** for Python (Rust, `rowan/`). It parses with
ruff (private names mangled as CPython does) and walks the syntax tree, running the program on
abstract values. Imports run as Python runs them (`TYPE_CHECKING` False, `import *`, namespaces).

* **Values** (`Value`, `domain.rs`) are a set of object ids plus a scalar part (`KeyVal`): a
  literal, up to 64 literals, the keys of some objects, or `Data(label)`, an unknown named by
  where it comes from (`row["id"]`, `x.shape[0]`). A label can also hold a tensor shape, or a
  builtin type (`typed_label`: `int(?)`; from annotations in general runs, builtins, operators).
* **Objects** (`Obj`, `interp/heap.rs`) are identified by allocation site and context (the call
  chain, 2 deep). A *single* object stands for one runtime object: stores into it are strong.
  Allocating a site again while its earlier object is live gives a variant (up to 8), then a
  *merged* object (stores only join). Garbage collection at returns, loop and try exits drops
  the state of objects made since (`Obj::born`, renewed by a strong re-allocation) nothing reaches.
  An open row's unread key holds a *placeholder* per object and key (`path_placeholder_value`, 3 deep); a
  read on more than 8 objects (`wide_read`) makes one per key there (weak; a library object's attribute: one unknown per name).
* **State** (`State`, `state.rs`) is one point on one path: names → values, and per object
  its **rows** (`(object, namespace)` → `Row`), slots (the value under each key), elements (scalars in
  `elems`, objects in the `Elem` slot: written together, `add_elem` / `set_positions`; missing: unknown),
  lengths (one writer, `set_lens`; a tensor dim after `T` rounds of one append each, `sym_lens`),
  and the requirements recorded so far. The maps are persistent Patricia tries (`layered.rs`): a copy is O(1), a join or diff costs what two states differ in.
* **The analyzer** (`Analyzer`, `interp/mod.rs`) is components with private state, one file each
  (Layout); models, the report and the driver see the `Heap`, their `m` and `api.rs`. Its doc sorts
  them for a call memo: running context (a key), state changed outside `State`, output, caches.
* **Rows** are the key-set model: `present` (may be there) ⊇ `likely` (there unless unknown
  code removed it) ⊇ `definite` (there on every path), plus `absent`, open `sources` (inputs),
  and `mutators` (unknown calls). A row without sources or mutators is *closed*; a successful read narrows the key in.
* **Statements.** A test is evaluated once into the states where it is true and false
  (`split`, `narrow.rs`: narrowing reads its parts' values, `traced`); both branches run, join;
  a pure test's branch rows come back when it recurs (`CondFact`: tests a function repeats, until a write).
  Loops over at most 16 known items are unrolled (comprehensions calling nothing per item: 64);
  others run to a fixpoint (40 rounds, then widened; verdicts are the last round's), making
  summaries; a target bound to one of the records a summary (or an input's element) stands for
  is a strong *view* per iteration (`bind_views`; `link_view`: changes join both ways). A
  running `try` (`TryCtx`) records the state at every point of its body and callees that may
  raise (a raise, a failing read, a model's raise, any unknown call: class `any_exc`), joined
  per class (an object a raise leaves half-built is dropped there unless held, `drop_unreached`);
  handlers start from the raises they may catch, the rest go on through the `finally` to the
  enclosing try. `with` is a `try` whose `finally` is `__exit__`; `finally` runs on every way out.
* **Calls** (`calls.rs`): every call of user code runs the callee's body at the call site, in a
  context of the innermost `chain` (2) call sites. **Budget:** each function code (all closures
  of one `def`) may run 4,096 statements in all (`budget`); past that its calls are unknown
  calls that may also change what its code reaches (`effects.rs`, by `flows.rs`'s reading of
  function texts: globals, cells, defaults and arguments whose value, or what it holds, the code
  may change or give away (to the program's functions: as they use it; to library functions that only read, given no code: nothing, `read_only_library`; a dict, list or set changed
  in place: its items), a local aliasing what it is bound to; class attributes by name, rebound
  where a store's base may be the class (a call's result: the classes it may return, `result_classes`); a receiver by the names its code uses unless it escapes,
  `receiver_effects`; one call for several receivers), and the function gets a reporting run (open
  parameters) for the calls that did not run. Entry runs are not counted (one per function): work is bounded by functions × budget.
  * **Recursion:** the inner call uses the running activation's partial result; the activation reruns from the heaps of its recursive calls too (`Recursion`), 8 rounds, then widened.
  * **Unknown values** are not taken for the program's instances: their methods are unknown calls. An instance unknown code changed may hold a name its class has (not a method or class).
  * **Unknown calls** may change everything they reach through what they are given, at any depth:
    records get a mutator, slots may hold anything (one shared unknown object, which no store
    changes: a value stored into it goes to unknown code, `escape`), lists and sets any items
    and length (NamedTuples, patterns, matches, paths, frozensets, enum members' name and value
    cannot change); the program's functions among it may run (`follow_code`). A walk from the same
    values (by any unknown code) redoes only what changed (`Havoc`'s memo). They return an unknown and may raise anything; its
    items may be those of the mappings it was given itself (not `**d`'s, the callable's own, an `int`'s; open through the call, not
    through a value read under an input's key: `unknown_result_rows`). `a + b` of unknowns: either's elements. Library functions
    known to have no effect are `noeffect` (`ALLOWLIST_PREFIXES`) unless they change their arguments (`random.shuffle`) or get
    the program's functions; several callees: `alternatives` (unknown values: one unknown call; a method on several receivers: one
    call per function when it would not run, or on demand; those none of which runs — out of budget, past the work on demand
    or the deadline — one `skipped_calls`).
  * **Builtins** run the dunders Python calls for them (`builtin_dunder`); one without a model is unknown code. A TypeError
    may come from those taken to fail only on wrong types, and from hashing a list, dict or set (`check_hashable`). Models,
    operators and subscripts on literals beside objects (`"a" if c else [1]`) run on each part (`split_mixed`).
  * **Entry points, general runs:** functions the main run does not call (also those only entry
    runs call) run with open parameters (`param F.x` schemas), from the module's end; a called
    function gets a requirement-only run so too. A *general run* (`Entries::general`: hover values
    for any call, apart from the program's calls, `Fact`) starts from `after_any_code`: the module's
    end after any of the program's functions ran (`follow_code` over all, a result as its callers
    use it, `caller_returns`; instances by name, `methods_may_have_run`), joined with the heaps at
    module code's calls (`note_call`); parameters as annotated (`list[dict[str, int]]`'s parts too: `Heap::ann`), a method on any instance (on demand, of a subclass past 64
    of them: any attributes), a classmethod on the class or a subclass. Facts on: a facts-only general run where needed.
* **Generators** run whole at their first consumption (`force_gen`), in the creating call's context; a consumer of item
  *k* sees the effects up to yield *k* (`GenEffects`; at a position not known, or one object for several runs, some yield's;
  joined in where the consumer changed them). If code between items (`note_resume`; unknown code: a key it made
  there-unless-removed too; deletes) changes what the generator reads after a yield (`Gens::watch`, also after the call
  that started it while held, or a global holds it), it runs again with that changed at each yield (`with_gen_restart`).
  `@contextmanager` bodies run at yields. A `for` over a user iterator (`__iter__` returns self) calls `__next__` before each
  iteration (`for_user_iterator`).
* **Requirements and schemas:** reading a key from a row open through an input records a
  requirement on that input: `always` (every path), `sometimes`; `only_if`: tests of the
  function's parameters as passed that all its readers passed (`State::path`, `guards.rs`). A
  read guarded by a `KeyError` handler counts (a frame column read in a `try` is a probe: not);
  one on one of several objects needs each one's only sometimes (an element's: per element).
  A general run's on its own parameters are a hover's contract (`Findings::contracts`); a parameter's fact at its `def` keeps what each call passed (`Fact::passed`, checked by its run's verdicts).
* **Verdicts:** every read, `del` and `pop` gets a tier per object from its row (`classify`): `proven`, `possibly`,
  `missing`, `input` are sound; `likely` / `likely-missing` optimistic (unknown code removed nothing). In one of
  several functions a value picks (`CONVERTERS[k](x)`, `k` unknown: `cx.picked`) a `missing` is `possibly`.
* **Models** (`models/`, by qualified name in `Registry`; free functions of `&mut Analyzer` that see the `Heap`, their
  `models::Tables` and the interpreter's public methods) compute CPython's result on known values (str / bytes, formatting,
  json, re, datetime, decimal, sqlite, csv…), and simulate pandas columns (an unknown value's mask / column-list / `.loc`
  selection holds its columns, its `drop(columns=…)` the others), pydantic / FastAPI validation and routing, and torch / numpy shapes
  (dims as polynomials; an undecided condition is a requirement and then a fact on the path; in torch modules `int` / `Tensor` annotations hold; a module-level unknown number names its dims, a module-level tensor's unknown dims are one value each).
* **Deadline:** a safety net (`ms`, 10 s): past it user calls are unknown, running ones stop; the budget bounds the work.
  **On demand** (`on_demand`, `general_run_of`): from `after_any_code`, a hovered function's general run with
  verdicts (fresh budget; past four budgets of work its callees' calls are unknown, `callees_cut`, several callees one `skipped_calls`,
  classes among them their instances made there; every run starts from the module's end, `demand_start`). `rowan serve` starts the whole run and the module code's
  (`__main__` block's calls skipped) together; the latter answers unless the former completes within `EARLY` (1 s), whose
  document then follows as an event. A changed file, a newer `analyse` of it or a `cancel` (the extension's, when a
  request times out) cuts the analyses running for it as the deadline does (`Budget::cancel`, read on its own thread).
* **Soundness** holds relative to the models and to annotations (of values whose class is unknown;
  a general run's `x: int` is an int, not a subclass overriding int's operators). `exec`, `setattr` /
  `getattr` with unknown names (known in part, an f-string's: the attributes whose names fit, `Payload::Affix`), metaclasses and
  monkey-patching (rebinding a function or class too; a module's function the program calls only by name has no other callers) are unknown or ignored;
  unknown code does not shadow a class attribute on an instance of a library-based class; an unknown beside objects is no instance
  of the program's or a library's classes (`isinstance`: a builtin's, by its type); a generator expression runs when made. List lengths are tracked up to 15 (a few per path: each one's positions;
  `xs[:n]` keeps xs's). An object a path has no facts of reads as unknown (`read_row`). A set's items have no order (`ordered_items`).

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (3.14 pinned), for the test harnesses.
* **New test cases are specifications.** Write the expected rows and verdicts by hand from the program text first,
  then make the engine produce them. Never derive an expectation from engine output. When an expectation turns out
  wrong, fix it with a comment saying why, keeping the line count. Value analysis matters more than bug-finding verdicts.
* **Hypotheses need a repro.** For a suspected bug or slowdown, a small program showing it first (a golden case; `#@work` / `#@tabulations` bounds for cost), fix it, then measure.
* **Soundness first.** Where the runtime oracle and this file disagree, the oracle wins. Golden cases matter; snapshot changes are fine when sound.
* **Commits.** Commit when a round of work is complete and golden, snapshots and `cargo test` pass; never commit a failing state. Review snapshot diffs before `--update`.
* **Plans.** When a request mixes several things, write the plan first, then work it in order. Keep this file under 200 lines and up to date with every change.
* **Output is for Python engineers.** The text report, hovers and diagnostics use words and Python access paths
  (`row["id"]`, `items[*].sku`), never engine terms (row, tier, source, placeholder) or internal site labels
  (`report::site_text` names unknown code by the call as written). A hover (`renderHover`): `name = value` (`(parameter) int(?)`; on a line that assigns it, the value assigned), then Requirements (of parameters, and inputs read on the line), In Calls, Allocation Sites (not one object made by the name's own assignment), Notes (only what is used above; last, a cut analysis). `vscode-rowan/test.js` checks real hovers.

## Commands

```
cd rowan && cargo build --release && cargo test --release --lib && cd ..
uv run --no-project python tests/run_golden.py      # all suites, ~5 s; paths select cases; -v
uv run --no-project python tests/run_golden.py --pending [--prune]   # cases in tests/*/PENDING
uv run --no-project python tests/run_snapshots.py   # --update only after reviewing the diff
uv run --no-project --with pandas --with pydantic --with pydantic-settings --with fastapi --with httpx --with numpy \
    python tests/oracle.py [--strict|--lenient] [-v] [-j N] [case…]  # vs real runs, ~1.5 min
uv run --no-project --with torch --with numpy python tests/torch_oracle.py [case…]
uv run --no-project python tests/fuzz.py [KIND…] [-n N] [--seed S] [--shrink]  # random programs vs CPython, also at budget=0; -n 50: ~5 s
cd vscode-rowan && node test.js && npm run package  # hovers; the .vsix (test_package.js X.vsix TARGET)
uv run --no-project python bench/run.py rich rowan/target/release/rowan "$(uv run --no-project python bench/prepare.py rich)"  # also email, stdlib12k, click; pip, timm, sktime, functional
uv run --no-project python bench/hovers.py rowan/target/release/rowan pip  # each function on demand (`rowan hovers F --only NAME`: one); tests/ondemand_check.py: vs whole runs
uv run --no-project python bench/first_hover.py rowan/target/release/rowan rich  # rowan serve: time to the first hover in a file
```

Run the oracle on the cases you touched, in full before a commit that changes models. CI (`ci.yml`) runs the above but
torch and the benchmark (`bench.yml`, also the `budget=0` golden count), and a packed .vsix per native target.

## Tests

* **Golden suites** `tests/{cases,torch,pandas,pydantic,fastapi,stdlib,apps,numpy}/`, ~1200 programs; library suites have `small/`, `models/` (torch `playground/`: the owner's), `PENDING`.
* **Directives** (full rules in `tests/CONVENTIONS.md`, also `tests/{torch,pandas}/`):
  `#@ tier key`, `#@row NAME: present= definite= absent= closed|open`,
  `#@value NAME: exactly= | unknown | includes=` (`#@general`: for any call), `#@schema src: …`, `#@only src: KEY if TEST | KEY none`, `#@todo …`, `#@argv`, `#@shape` (torch). Cost bounds, by hand:
  `#@objects`, `#@tabulations`, `#@joins`, `#@work`, `#@visits`, `#@havoc <= N`; `#@limits k=v,…` sets the case's `ROWAN_LIMITS`. A directive-only line applies to the code line above.
* **Oracle** (`tests/oracle.py`): reruns each case under CPython (a tracer), checks every directive (schemas by dropping keys), strict on library suites. **Fuzzing** (`tests/fuzz.py`): random programs in 11 areas (lists, dicts, attributes, try, generators, formatting, calls…); the values, key sets and access outcomes of CPython's runs must be among rowan's claims (`--shrink`: a repro).
* **Snapshots** (`tests/snapshots/`): `--json` on `examples/etl_report.py`, `tests/fixtures/proto_*.py`: the guard for internals (ids may shift). **Unit tests**: CPython values, the regex engine, tries.

## Debugging switches

* `ROWAN_DEBUG=1`: body runs (`run F at SITE`), skipped calls, entry runs, a `work:` line of counters (statements, units = statements + walk visits / 16, body runs, objects, row joins, visits, havoc visits; ~25% slower). `2`: raises.
* `ROWAN_LIMITS=budget=4096,ms=10000`: the only knobs (golden `#@limits` per case); `ROWAN_FOLLOW=pkg,…` is `--follow`; `ROWAN_TODO=check`.
  Profiles: the bench workflow's `perf`; locally `xctrace record --template 'Time Profiler'` (symbols: `atos -l` with the binary's slide; `sample` hangs on macOS 27); time inside the guard (`.guard/run /usr/bin/time -l`).

## Layout (`rowan/src/`, ~69k lines: the engine ~26k, the models ~42k)

| file | role |
| --- | --- |
| `intern.rs`, `domain.rs`, `layered.rs`, `state.rs`; `frontend.rs`, `scopes.rs` | interned keys, sources, lenses; `Row`, `KeyVal`, `Value`, transfers, `classify`; persistent maps (`LayerMap`, `TwoLevel`); `State`, joins, `diff_keys`; ruff parsing, leaked `&'static` trees, module registry; lexical scopes |
| `interp/mod.rs`, `context.rs`, `program.rs`, `entry.rs`; `heap.rs`, `alloc.rs`, `gc.rs` | `Analyzer` and its components' map; `Context` (frames, callers, call chain), `Modes`; `Program` (paths, models, modules), imports; running the program, entry runs (`Entries`); the `Heap` (objects, payloads, lengths, class attributes, keys for objects); allocation policy (`alloc`: variants, recency); liveness (`live_among`, `gc_pinned`), collectors, `rename_recent` |
| `interp/stmts.rs`, `loops.rs`, `classes.rs`, `tries.rs`; `exprs.rs`, `ops.rs`, `compare.rs`, `comps.rs` | statements, match; loops (unrolled, fixpoint); class definitions; raises, try contexts (`Unwind`), try / with, `finally`; expressions; operators, f-strings; comparisons, truth; comprehensions |
| `interp/funcs.rs`, `gens.rs`, `narrow.rs`, `guards.rs`; `access.rs`, `items.rs`, `attrs.rs`, `keys.rs` | functions, names, iteration; generators and `@contextmanager` bodies (`Gens`); tests (`split`), narrowing; tests of parameters on a path; what accesses share (guards, requirements); item access; attribute access; keys standing for values |
| `interp/calls.rs`, `recursion.rs`, `budget.rs`, `unknown.rs`, `effects.rs`, `flows.rs` | calls: per-call-site runs, `__new__`, dataclass init, bindings; `Recursion` (activations, fixpoint rounds); `Budget` (per code, deadline, work counters); unknown calls (`Havoc` walks); what skipped calls may change (`skipped_call`, `EffectsCache`); what a function's text does with values (`Flow`: aliases, escapes) |
| `interp/findings.rs`, `names.rs`, `report.rs`, `api.rs`; `main.rs` | `Findings` (verdicts, notes, row checks, hover facts), `Names` (symbols, origins); text / JSON / facts output; what models, report and driver may use; CLI (mimalloc), `rowan serve` (on a 1 GiB-stack thread) |
| `models/` | `mod.rs` (`Registry`, builtins, containers, `Tables`); `tables` (what builtins / library calls raise, change, keep); CPython-exact values (`pyvalues`, `pytext`, `pynum`, `chrono`, `pathtext`); `re` (`pyre`, `remodel`); stdlib (`enums`, `exceptions`, functools, itertools, …); `pandas`, `pydantic`, `fastapi`, `torch`; `shape.rs` (tensor dims as polynomials) |
| `rowan/py/`, `vscode-rowan/` | a PyO3 binding (does not load on macOS 26); the editor extension (talks to `rowan serve`) |

## Implementation rules that are easy to break

* **Models allocating several objects at one node** must give each its own context.
* **Callees** run with the caller's `state.exits` taken out and restored (y438).
* **GC roots.** A side table keyed by objects goes into `side_edges`; anything else the analyzer holds (a running with's managers) into `gc_pinned`: else a read gets an empty row (and a list no length).
* **Generator yields** use `alloc_raw` for per-yield states. **Stdlib objects** (`stdobj.<kind>`): Attr slots, `m_stdobj`.
* **Attributes resolved by class** (`class_resolves`): class, model and `__getattr__` objects have more than their rows show.

## Measured (release build, Apple M-series)

| input | time |
| --- | --- |
| golden / oracle | ~5 s / ~1.5 min |
| stdlib12k (12k lines) / email (10k) / rich (27k) / click (10k), no deadlines | ~2.7 / ~2.7 / ~8.7 / ~6.0 s; 0.13 / 0.07 / 0.18 / 0.10 GB (`--facts`: ~4.7 / 2.5 / 8.9 / 6.4 s) |
| pip (134k) / timm (132k) / sktime (355k) / functional (213k), no deadlines | ~12.4 / ~24 / ~13.5 / ~5.9 s; 0.31 / 0.80 / 0.29 / 0.34 GB |
| on demand (`bench/hovers.py`, every function): rich / pip / timm / sktime | setup (every module) 0.23 / 8.7 / 5.0 / 28 s; per function p50 ≤ 7.6 ms, p99 0.16-0.45 s, max 0.5 / 0.5 / 0.7 / 3.3 s; first hover in a file (`bench/first_hover.py`) p50 1.0 / 1.0 / 3.7 / 0.2 s, max 1.2 / 4.8 / 3.8 / 1.1 s; `rowan serve` on each timm model file ≤ 0.9 GB |

## Known problems and next steps

Open items and ideas ruled out are in the pull requests' descriptions (Remaining, Ideas not pursued). Parked: `parked/joined-summaries` (unsound), `slotmap-trie`, `parked/memo-probe`, `parked/suggested-changes` (the retired list of open items).
