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
uv run --no-project python tests/run_golden.py      # 694/694 in ~2.5 s (417 + 162 torch + 115 pandas); paths select; -v
uv run --no-project python tests/run_golden.py --pending [--prune]   # cases in tests/{torch,pandas}/PENDING (empty)
uv run --no-project --with torch --with numpy python tests/torch_oracle.py  # torch expectations vs real PyTorch
uv run --no-project --with pandas python tests/oracle.py [--strict|--lenient] [-v] [case…]  # rows / values / verdicts / schemas vs a real run
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
| `models/{mod,pandas,minire}.rs` | library models (`Registry::lookup`); builtins (`len`, `sum`, `bool`, `sorted` by literal keys, `next` with cursor…), dict/list/str methods, `copy`, `dataclasses`, `collections`, `json`, `csv`, `argparse`; pandas (`df_dispatch`, `groupby_method`, `series_method`; see below); a small regex matcher. |
| `shape.rs`, `models/torch.rs` | tensor shapes: dim polynomials over atoms, shapes with rests and assumptions, constraints, path facts (`State.tf`), printing (`PrintCtx`: scopes, self aliases, hover names); the torch model (ops, `nn` layers, functional, `nn.Module`, requirements / errors, hover rendering). |
| `report.rs` | dedupe, schema table, text / JSON lines / `--golden` / `--facts` (`text` = rendered value). |
| `main.rs` | CLI, `serve_loop`, `run_with_deps`, site-packages discovery; analysis on a 1 GiB-stack thread. |
| `rowan/py/` | PyO3 binding; the wheel does not load on this machine (see problems). |
| `tests/torch/`, `tests/shapes.py`, `tests/torch_oracle.py` | 145 small torch programs + 17 models (`CONVENTIONS.md`: directives `#@shape`, `#@dim`, `#@requires`, `#@shape-error`, canonical forms); `PENDING` lists cases not passing yet (empty); the oracle runs each case under real PyTorch. |
| `tests/pandas/`, `tests/oracle.py` | 100 small pandas programs + 15 pipelines (`CONVENTIONS.md`); the oracle traces each case under real pandas / Python: rows, values, verdicts (`possibly` must show both in strict mode), schemas by dropping one `read_csv` column; strict on `tests/pandas`, lenient (checks what runs) on `tests/cases`. |
| `tests/run_golden.py`, `tests/cases/` | 417 golden cases. Directives: `#@ tier key [; …]` (verdict on that line's access; tier `sound` = anything but `missing`), `#@row NAME: present={..} definite={..} closed|open` (row of NAME on that line), `#@value NAME: exactly={..}|unknown|includes={..} [; NAME2: …]` (the name's value), `#@schema src@L<n>: always={..} sometimes={..} only={..}`, row floors `possible={..}` / `definite_within={..}`; a row on a line run several times (calls, entry runs) is the join of the runs, `#@todo …` (reported, not checked), `#@argv …` (per-case flags, relative paths against the case dir). A package case is a directory whose entry shares its prefix (`k94_pkg/k94_modules.py`); other files there are modules. |
| `tests/run_snapshots.py`, `tests/snapshots/`, `tests/fixtures/` | `--json` snapshots on the example and three old-prototype modules (inputs only); the regression guard for internals. |
| `vscode-rowan/` | extension: talks to `rowan serve` (falls back to the CLI), hover = facts for (line, name), verdicts as diagnostics. |

Cases: `a01`–`e36` design, `f37`–`n97` AST / objects / packages (`m96_pkg`: 1,700 lines),
`o98`–`w315` one-hypothesis rounds, `x316`–`x368` known weaknesses (`#@todo` + floors),
`y369`–`y418` inside functions. Every case was written as a value specification *before*
running the engine; wrong expectations carry a comment saying why. Decided on known values,
`?` on unknown ones; `json.loads` results are open unless the text is a literal.

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
* **Object identity.** Allocation site = module-prefixed line:col (+ class, `#i` in unrolled
  loops) and a context: the call chain (hashed beyond `chain` levels), `o<receiver>/<site>`
  for methods, `sum<f>v<n>` in summaries. Hitting a live site again makes the object weak;
  recency folds loop allocations. Content under an unknown key is a weak `Kind::Unknown`
  path object.
* **Positions.** Literal containers, straight-line appends, decided comprehensions, sorts by
  literal keys, known slices, generator yields, `zip`/`enumerate`/`map`, list mutators, `a + b`,
  `range` keep positions (unroll 64 in torch programs, else 16); unknown-count appends keep
  `known_prefix`. Dicts keep insertion order (`RowData.order`); one-shot iterators exhaust;
  `True`/`1`/`1.0` are one key (`canon_key`).
* **Values.** Arithmetic, comparisons, formatting, string methods, membership, `and`/`or`/
  `not`/`IfExp`, `is None`, `any`/`all`, `int`/`float`, `reduce`, `filter` on known values;
  `while` tests decided at the head unroll; `match` decides literal / singleton / or / capture
  / mapping patterns; `__bool__`/`__len__` decide truth (`truth_of`). Branch facts
  (`CondFact`): the join of an `if` on a pure test, of a try setting a flag, of a `for` over a
  name records each side's rows, restored by a later identical test while its names, guards
  and rows are unchanged. `State.name_keys`: a store / `setdefault` / an `in` test through a
  name holding several objects makes the key definite for that name. Row checks and hover
  facts in summaries are `Obs` / `FactObs` resolved per instantiation (and skipped after a
  statement that raised). Unknowns print as *symbols* (`symbol_scope`, `report::in_scope`,
  `--facts` legends). Entry runs (uncalled functions, closures, lambdas; up to 4 rounds) get
  open parameters. A summary reads a rebound module name as a deferred load per call site.
* **Classes.** Dataclass `__init__` (subclasses, `super()`, `__post_init__`), properties,
  classmethods, class decorators, C3 MRO, `__init_subclass__`, `__getattr__`/`__setattr__`,
  descriptors, `__iter__`, `__missing__`, `__dict__`/`vars()` (`ViewKind::Attrs`), Enum,
  `cached_property`, `lru_cache` (literal arguments); field classes (`ctor_field_classes`,
  `infer_field_classes`) type attribute placeholders; a dict/list method name on a class-less
  placeholder uses the container model plus a dispatch event. `namedtuple`, `itemgetter`,
  `attrgetter`, `re.match` are small generated modules analysed like imported code.
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
  `yield` (`CmHook`). A callee's deferred access that may fail raises into a try around the
  call (from the pre-call state; inside another summary it is re-deferred as guarded); raise
  states are never taken from inside a callee's summary (`try_sum_depth`). `if k not in d:
  raise …` makes k a requirement of the inputs (`require_by_raise`). `is` on distinct objects
  is False; a builtin container never equals a scalar; literal strings slice.
* **Imports.** Local modules / packages (`ModuleInfo::pkg`), roots = file dir, the dir above
  its outermost package, `--search-path`; `--follow` packages unless a model covers them.
* **Tensors** (SPECIFICATION §2.3, `tests/torch/CONVENTIONS.md`). A tensor is a
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
* **pandas** (SPECIFICATION §11.2, `tests/pandas/CONVENTIONS.md`). Frames: Kind::Frame,
  columns in the Item row (tuple keys = MultiIndex); meta slots: index levels (`index`, a
  literal tuple; unknown unless set: `fresh_index`, `derive`, `copy_meta`), labels / row count
  / per-column values (`Known` may/sure) and dtype (`int float num bool object`), `colname`.
  `derived(…, Rows::{Same,Subset,Other}, with_vals)` carries them. Tracked Series are Dict
  objects labelled `series@` / `row@` (`is_series_obj`): keys = index labels, meta name /
  levels / values; `series_getattr` routes methods, `.str`/`.dt` accessors are `Locidx`. In
  pandas modules (`pandas_on`) calls / attributes / list subscripts on placeholders, paths
  below them, symbolic frames and entry constants record `concrete` events (a literal binding
  decides one), so pandas code in functions is tabulated. Row views (iterrows / apply / records)
  never narrow their frame; an open merge side's sources get `Kill` of the other side's
  definite columns. Guarded accesses record `guarded` verdicts (golden only, never diagnostics).
* **Budgets.** `MAX_MS = 2000` (then no new tabulations / clones); 256 tabulations per function /
  call, 256 clones per site, `MAX_ACTIVE` 4, 3 summary rounds, unroll 16, path 3.
* **Server.** `rowan serve` caches `--facts` per file with its modules' content hashes;
  parse trees per (path, hash); summaries are not reused across runs.

## Measured (release, Apple M-series)

| input | time | notes |
| --- | --- | --- |
| golden suite (417 + 162 torch + 115 pandas) | 2.5 s | one process per case; oracle on all 532 cases ~3 min |
| `j93` (830 lines) / `m96_pkg` (1,716 lines) / `proto_summary.py` (1.2k, self-referential) | 0.09 s / 0.10 s / 2.6 s (deadline) | all pass / all pass / 0 ✗, 28 ⚠ |

## Known problems and limits

* PyO3 wheel: fails to load on macOS 26 / Xcode 26 (dyld LINKEDIT); CLI / extension skip it.
* Large self-referential programs are noisy; precision on real repositories never measured.
* A store through a name holding several objects is weak on each (golden `h72`, `maybe`;
  `x346`/`x347` keep the precise answer as `#@todo`).
* An unknown call whose arguments hold records may alias them; its attribute row is open.
* Mutual recursion widens after 3 rounds; `while` loops with unknown conditions join;
  comparisons on unknown values keep both branches; `match` class / sequence patterns stay
  undecided; `getattr` needs a known name.
* pandas: a column stored on one branch of an open frame is an `always` requirement (rows do
  not tell a store from a narrowing); no relations between columns (`note` absent ⇒ `note_y`);
  absent keys of open frames are listed only when seen (lenses rule out more); a read_csv index
  is not known to be a RangeIndex; names holding several frames narrow only by name.
* `dict(zip(keys, row))` with an unknown-length row makes every key definite (`d30`); a
  broken-off `for` exhausts its iterator; `ChainMap` is a copy; ASTs leak per file version.
* Torch: dtype / device not modelled; numpy only the basics; `einsum` letters broadcast;
  element counts of `view` checked only when literal; an unknown-count `Sequential` is the
  join of its input and one module's output; facts from `!=`/`<` tests are not learned.

## Where to go next

Open items, in the order they seem worth doing: (00) pandas: stored-vs-narrowed keys in rows
(sometimes requirements), the `#@todo`s in `tests/pandas`; (0) torch: shape facts from `<`/`!=`,
dtype tracking, substitution-based instantiation if tabulation gets slow on big models;
(1) summary reuse across runs in the server;
(2) the weak store through several objects when the name read differs from the one stored
through (`x346`, `x347`); hover values in resolved function facts; (3) precision on self-referential programs and a runtime oracle on real repos;
(4) more cases written before running the engine. Keep this file under 200 lines.
