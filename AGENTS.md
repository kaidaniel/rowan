# AGENTS.md — rowan, handoff notes

An abstract interpreter for Python: every name holds a set of possible values (literals,
objects, `?` unknown); reports what names hold, accesses that can fail, and the schemas the
program demands of its inputs. Key sets of dicts / instance attributes (`Row`) are one model
among others; tensor shapes (PyTorch) are another. Design: `SPECIFICATION.md` (§2 values and
models, §2.2 notation, §2.3 tensors; §9.8 and §16 note where the engine went further).
User-facing: `README.md`. This file is for whoever picks the work up; true at the last commit.

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (`.python-version` pins 3.14); never
  the system or a local interpreter. The Python side is only the test harnesses.
* New test cases are *specifications*: write the expected rows and verdicts by hand from
  the program text first, then make the engine produce them. Never derive an expectation
  from engine output; when one turns out wrong on re-reading, fix it with a comment saying
  why. Value analysis (which keys, which columns) matters more than bug-finding verdicts.
* Commit without being asked whenever a round of bug fixes or a feature is complete and the
  tests pass (golden, snapshots, `cargo test`), one commit per round / step so any of them
  can be reverted; never commit a failing state. Commit messages end with a
  `Co-Authored-By: Claude <model> <noreply@anthropic.com>` line naming the model at work.
* Work the plan in order, with a written plan first when a request mixes several things;
  if time remains, write more example programs that probe specific hypotheses about bugs.
* Keep this file under 200 lines and up to date with every change.

## Commands

```
cd rowan && cargo build --release && cargo test --release --lib && cd ..
rowan/target/release/rowan FILE.py [--json|--golden|--facts] [--no-rows --no-summaries --no-entry --no-imports] [--value-depth N]
                                   [--search-path DIR]… [--follow PKG]… [--site-packages DIR]…
rowan/target/release/rowan serve                    # JSON lines over stdio (README has the protocol)
uv run --no-project python tests/run_golden.py      # 579/579 in ~2 s (417 cases + 162 torch); paths select cases; -v
uv run --no-project python tests/run_golden.py --pending [--prune]   # torch cases listed in tests/torch/PENDING
uv run --no-project --with torch --with numpy python tests/torch_oracle.py  # torch expectations vs real PyTorch
uv run --no-project python tests/run_snapshots.py   # SAME on 4 snapshots (fixed caps, no deadline); --update after review
cd vscode-rowan && node test.js && npm run package  # hover rendering check; rebuilds rowan-0.1.0.vsix
```

Env: `ROWAN_DEBUG=1|2` (tabulation reasons, summaries, budgets; 2: every event, raise, try
decision), `ROWAN_NO=a,b` (switch refinements off: `rg 'disabled\("' rowan/src`),
`ROWAN_LIMITS=ms=2000,tab=256,call=256,clones=256,chain=2,path=3` (soft deadline, then hard
caps), `ROWAN_FOLLOW=pkg,…`, `ROWAN_SUMMARIES=0` (old per-call-site engine, comparison
only), `ROWAN_TODO=check` (the harness checks `#@todo` expectations too).

## Layout (Rust engine `rowan/`, ~20k lines)

| file | role |
| --- | --- |
| `intern.rs` | interners (keys, strings, sources, lenses → ids); `IdSet<T>` sorted `Arc<[T]>` with merge-based set ops. |
| `domain.rs` | `Row` (present/definite/likely/absent, sources, mutators, terms; `Arc` + cached hash), `KeyVal` (Lit/Lits/KeysOf/Data/Bot/Param), `Value` = objects + KeyVal, `classify`, lenses. |
| `layered.rs`, `state.rs` | persistent layered maps (O(1) copy, O(diff) join/same/footprint); `State` (env, narrowed, rows, slots, elems, reqs, ret, exits). |
| `frontend.rs` | ruff parsing, leaked `&'static` trees, node→module registry, parse cache per (path, hash), `site_prefix` (non-main modules prefix their sites with `M<idx>:`). |
| `scopes.rs` | lexical scopes (locals / cells / free / globals). |
| `interp/mod.rs` | `Analyzer`, `Options`, `Obj` (`single`, `literal_len`, `known_prefix`, `copied_from`…), `ModuleInfo` (`pkg` for relative imports), allocation (`alloc_full`: site + ctx, `#i` unroll suffix, weak on live collision), budgets, `debug_level`, facts. |
| `interp/{stmts,exprs,funcs,narrow,access,calls}.rs` | statements (if/while/for/try/with/raise/imports), expressions (arithmetic and comparisons on literals, `%` and f-string formatting, comprehensions with positions), functions / targets / iteration / generators / copies, narrowing, record and attribute access (placeholder method resolution, field classes), calls (bindings, star forwarding, tabulation vs summary, call cache, contexts, dataclass init, classmethods). |
| `summary.rs` | placeholders (`PH`: kind, cls, `exact`), events, `summarise_sig` (per key variant: function, code cells, typed cells, argument classes; its own allocation context), footprints, `Inst` (instantiation: resolve, path_value, clones, deferred stores, appends), `propagate_events`, `should_tabulate` (budgets). |
| `models/{mod,pandas}.rs` | library models (`Registry::lookup`); builtins (`len`, `sum`, `bool`, `sorted` by literal keys, `next` with cursor…), dict/list/str methods, `copy`, `dataclasses`, `collections`, `json`, `csv`, `argparse`, pandas. |
| `shape.rs`, `models/torch.rs` | tensor shapes: dim polynomials over atoms, shapes with rests and assumptions, constraints, path facts (`State.tf`), printing (`PrintCtx`: scopes, self aliases, hover names); the torch model (ops, `nn` layers, functional, `nn.Module`, requirements / errors, hover rendering). |
| `report.rs` | dedupe, schema table, text / JSON lines / `--golden` / `--facts` (`text` = rendered value). |
| `main.rs` | CLI, `serve_loop`, `run_with_deps`, site-packages discovery; analysis on a 1 GiB-stack thread. |
| `rowan/py/` | PyO3 binding; the wheel does not load on this machine (see problems). |
| `tests/torch/`, `tests/shapes.py`, `tests/torch_oracle.py` | 145 small torch programs + 17 models (`CONVENTIONS.md`: directives `#@shape`, `#@dim`, `#@requires`, `#@shape-error`, canonical forms); `PENDING` lists cases not passing yet (empty); the oracle runs each case under real PyTorch. |
| `tests/run_golden.py`, `tests/cases/` | 417 golden cases. Directives: `#@ tier key [; …]` (verdict on that line's access; tier `sound` = anything but `missing`), `#@row NAME: present={..} definite={..} closed|open` (row of NAME on that line), `#@value NAME: exactly={..}|unknown|includes={..} [; NAME2: …]` (the name's value), `#@schema src@L<n>: always={..} sometimes={..} only={..}`, row floors `possible={..}` / `definite_within={..}`; a row on a line run several times (calls, entry runs) is the join of the runs, `#@todo …` (reported, not checked), `#@argv …` (per-case flags, relative paths against the case dir). A package case is a directory whose entry shares its prefix (`k94_pkg/k94_modules.py`); other files there are modules. |
| `tests/run_snapshots.py`, `tests/snapshots/`, `tests/fixtures/` | `--json` snapshots on the example and three old-prototype modules (inputs only); the regression guard for internals. |
| `vscode-rowan/` | extension: talks to `rowan serve` (falls back to the CLI), hover = facts for (line, name), verdicts as diagnostics. |

Cases: `a01`–`e36` design, `f37`–`i92` AST / objects / patterns, `j93` an 830-line system,
`k94`–`n97` packages and site-packages (`m96_pkg`: 1,700 lines), `o98`–`w315` rounds of
one-hypothesis cases, `x316`–`x368` known weaknesses (`#@todo` + soundness floors),
`y369`–`y418` inside functions, `z419` the owner's. Every `h`+ case was written as a value
specification *before* running the engine; wrong expectations carry a comment saying why.
Convention: decided on known values, `?` on unknown ones; `json.loads` results are open
unless the text is a literal.

## How the engine works (the parts not in the spec)

* **Summaries + tabulation.** A function body is analysed once per *key variant* against
  placeholders and instantiated per call site; a call site is re-analysed concretely
  ("tabulated") when a recorded *event* of the summary is decidable there: dispatch on a
  placeholder, truth / `in` / `isinstance` / iteration / `.get` / arithmetic / formatting on a
  parameter, a parameter-keyed load from a closed literal dict, a forwarded `*args`/`**kwargs`.
  Nested summaries hand their events up (`propagate_events`). The signature part of the key
  is the classes of the receiver and of instance-bound parameters (a typed placeholder
  receiver contributes its class); such a class is *exact* (no dispatch event for subclass
  overrides). Summaries are also keyed by closure cells (code cells, typed cells).
* **Object identity.** Allocation site = module-prefixed line:col (+ class for instances,
  `#i` inside unrolled loops) and a context: the call chain (`L10:4/L7:11`, hashed beyond
  `chain` levels so depth never collapses activations), `o<receiver>/<site>` for methods,
  `sum<f>v<n>` inside summary variant n. Hitting a site again while the earlier object is
  live makes the object weak; recency folds loop allocations at the back-edge. The content
  under an unknown key (`xs[i]`, `d[k]` with unknown i/k on open objects) is a weak
  `Kind::Unknown` path object: stores on it are weak and join with the unknown prior.
* **Positions.** Literal containers, straight-line appends (a decided `if` is straight),
  `extend`, decided comprehensions, `sorted` by literal keys, `reversed`, known slices,
  generator yields (`next()` cursor), `zip`/`enumerate`/`map`, `insert`/`del`/`pop`/
  `clear`/in-place `sort`/`reverse`, `a + b`, `a * n`, starred targets, `range` (up to 64)
  keep positions (64 in torch programs, else 16); unknown-count appends keep the prefix
  (`known_prefix`). Dicts keep insertion order (`RowData.order`); one-shot iterators
  (`Obj.iterator`) exhaust; `True`/`1`/`1.0` are one key (`canon_key`).
* **Values.** Arithmetic, comparisons, formatting, string methods, membership, `and`/`or`/
  `not`/`IfExp`, `is None`, `any`/`all`, `int`/`float` (ValueError on non-numbers), `reduce`,
  `filter` on known values; `while` tests decided at the head unroll; `match` decides
  literal / singleton / or / capture / mapping patterns; `k in d` on closed records prunes.
  `__bool__`/`__len__` decide truth at control flow (`truth_of`). Branch facts
  (`State.facts`, `CondFact`): the join of an `if` on a pure test (names, literals,
  comparisons, `in`, `is`, `not`), of a try whose two exits set a flag differently, and of a
  `for` over a name / `range(name)` records each side's rows; a later identical test restores
  them while the test's names, the objects it reads (`guards`) and the rows are unchanged
  (rebinding, unknown calls and deletes drop facts; tests on lists / sets give none).
  `State.name_keys`: a store / `setdefault` through a name makes the key definite for that
  name's object, whichever it is. Row checks and hover facts inside a summary body are
  `Obs` / `FactObs` kept with the summary and resolved at every instantiation (hover values
  there show as captured symbols). Unknowns print as *symbols* (`Analyzer::symbols` /
  `symbol_scope`, set where they are made: parameters, placeholders, read paths, unknown
  calls, inputs; `expr:` data for `1 + b`); a symbol prints only in its function
  (`report::in_scope`), and `--facts` carries a `legend` per fact. Entry runs (functions nobody calls, also closures and lambdas made but
  not called, up to 4 rounds) get open parameters: defaults are `default | anything`, `*args`
  / `**kw` unknown. A summary reads a module
  name that is rebound anywhere (`Scopes::rebound`) as a deferred load per call site.
* **Classes.** Dataclass `__init__` (also for subclasses, `super().__init__`,
  `Base.__init__(self,…)`, `__post_init__`), properties with setters, classmethods bound to
  the accessed class, class decorators, `super()` chains, dataclass fields as instance data,
  `ctor_field_classes` + `infer_field_classes` (fields assigned `Cls(...)`, `{}`, `[]`,
  `defaultdict(...)`, annotations) typing attribute placeholders; a dict/list method name on
  a class-less placeholder uses the container model alone plus a dispatch event. C3 MRO
  (`mro`; `super()` follows the receiver's MRO), `__init_subclass__`, `__getattr__`,
  `__setattr__` / `object.__setattr__`, descriptors (`__get__`/`__set__`/`__set_name__`),
  `__iter__`, `__missing__` on dict subclasses, `__dict__`/`vars()` as the attributes
  (`ViewKind::Attrs`, `attr_mirrors`), Enum members (`enum_members`), `cached_property`,
  `lru_cache` (`memo_results`, literal arguments only). `namedtuple`, `itemgetter`,
  `attrgetter` and `re.match` are small generated modules analysed like imported code.
* **Exceptions.** `raise`, unknown calls and raising callees mark the enclosing try /
  call; a try body ending in `return`/`break`/`continue` did not raise; a guarded access
  that is definitely missing raises there (the handler starts from the states at the
  raising accesses, `raise_states`); safe builtins do not count; a `with` whose `__exit__`
  returns True swallows; a call whose every path raises leaves the caller unreachable.
  A `raise` (also inside a callee, `try_marks`) records its state and exception value
  (`raise_values`: a raised instance, or a KeyError with the key in `args`) for the
  handler's `as` name; a bare `raise` re-raises the handled names (`handling`). `finally`
  runs on the normal path, on the try's returns before they leave, and on an escaping
  exception a `return` in it swallows. `@contextmanager` bodies run at the generator's
  `yield` (`CmHook`).
* **Imports.** Local modules / packages (relative imports via `ModuleInfo::pkg`), roots = file
  dir, the dir above its outermost package, `--search-path`; `--follow` packages from
  site-packages unless a model covers them.
* **Tensors** (`TORCH_PLAN.md` decisions log, `tests/torch/CONVENTIONS.md`). A tensor is a
  `Data` label `\x01T<shape>` (`\x01TB` bool, `…~<dim>` tensor-or-int for arithmetic on
  unknowns), a symbolic int `\x01D<dim>`, a number `\x01N`, an unknown-rank size `\x01S`;
  everything else sees unknown data. Unknowns used as tensors have the shape `[*x.shape]` of
  their symbol (unknown call results: the variable bound to). Operations check conditions:
  decided false → `shape_errors` (the op raises), undecided → `shape_reqs` + a fact in
  `State.tf` (ranks, dim substitutions oriented literal > named > earlier parameter; linear
  equations solved; joins keep only common facts). Dims that differ at a join become a
  `Join` atom (`?`), stable across loops. In a module that imports torch (`torch_modules`)
  every function is tabulated (never summarised) and also gets an entry run; the entry
  instance's `__init__` gets open arguments in its own scope (defaults open), and dims print
  through the shortest self-attribute path holding them (`t_print_ctx`). Hover: `Tensor[B,
  T, 4*C]` with dims named by locals / letters, the line's requirements, and the calls'
  shapes as examples (`Fact.primary`).
* **Budgets.** `MAX_MS = 2000` (then no new tabulations / clones); caps: 256 tabulations per
  function / call, 256 clones per site, `MAX_ACTIVE` 4, 3 summary rounds, unroll 16, path 3.
* **Server.** `rowan serve` caches `--facts` per file with the content hashes of the modules
  it loaded; parse trees per (path, hash); summaries are not reused across runs. `m96_pkg`:
  160 ms first, 8 ms unchanged, 125 ms after an edit.

## Measured (release, Apple M-series)

| input | time | notes |
| --- | --- | --- |
| golden suite (417 + 162 torch cases) | 2.0 s | one process per case; torch models ~0.02 s each |
| `j93` (830 lines) / `m96_pkg` (1,716 lines) / `proto_summary.py` (1.2k, self-referential) | 0.09 s / 0.10 s / 2.6 s (deadline) | all pass / all pass / 0 ✗, 28 ⚠ |

## Known problems and limits

* PyO3 wheel: fails to load on macOS 26 / Xcode 26 (dyld LINKEDIT); CLI / extension skip it.
* Large self-referential programs are noisy (sibling closures' cells via summaries, dispatch
  over `self.objects`); precision on real repositories never measured (robustness: 137
  local scripts, no failures).
* A store through a name holding several objects is weak on each (golden `h72`, `maybe`;
  `x346`/`x347` keep the precise answer as `#@todo`).
* An unknown call whose arguments hold records may alias them; its attribute row is open.
* Mutual recursion widens after 3 rounds; `while` loops with unknown conditions join;
  comparisons on unknown values keep both branches; `match` class / sequence patterns stay
  undecided; `getattr` needs a known name; pandas columns from data stay open.
* `dict(zip(keys, row))` with an unknown-length row makes every key definite (`d30`); a
  broken-off `for` exhausts its iterator; `ChainMap` is a copy; ASTs leak per file version.
* Torch: dtype / device not modelled; numpy only the basics; `einsum` letters broadcast;
  element counts of `view` checked only when literal; an unknown-count `Sequential` is the
  join of its input and one module's output; facts from `!=`/`<` tests are not learned.

## Where to go next

Open items, in the order they seem worth doing: (0) torch: shape facts from `<`/`!=`,
dtype tracking, substitution-based instantiation if tabulation gets slow on big models;
(1) summary reuse across runs in the server;
(2) the weak store through several objects when the name read differs from the one stored
through (`x346`, `x347`); hover values in resolved function facts; (3) precision on self-referential programs and a runtime oracle on real repos;
(4) more cases written before running the engine (rounds r–w found 75 bugs, mostly in
models; x found 5 wrong answers). Keep this file under 200 lines.
