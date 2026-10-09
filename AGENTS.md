# AGENTS.md — rowan, handoff notes

Static key-set inference for Python records: for every dict-like value (dicts, DataFrame
columns, object attributes, kwargs, JSON/CSV rows, argparse namespaces…) which keys it has
at each point, which keys the program demands of it, and inferred schemas for external
inputs. Design: `SPECIFICATION.md` (17 sections + Appendix A; §9.8 and §16 note where the
engine went further). User-facing: `README.md` (notation, CLI, server, report). This file
is for whoever picks the work up. Everything here was true at the last commit.

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (`.python-version` pins 3.14); never
  the system or a local interpreter. The Python side is only the test harnesses.
* New test cases are *specifications*: write the expected rows and verdicts by hand from
  the program text first, then make the engine produce them. Never derive an expectation
  from engine output; when one turns out wrong on re-reading, fix it with a comment saying
  why. Value analysis (which keys, which columns) matters more than bug-finding verdicts.
* Commit without being asked whenever a round of bug fixes or a feature is complete and the
  tests pass (golden, snapshots, `cargo test`), one commit per round / step so any of them
  can be reverted; never commit a failing state. Commit messages end with
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
* Work the plan in order, with a written plan first when a request mixes several things;
  if time remains, write more example programs that probe specific hypotheses about bugs.
* Keep this file under 200 lines and up to date with every change.

## Commands

```
cd rowan && cargo build --release && cargo test --release --lib && cd ..
rowan/target/release/rowan FILE.py [--json|--golden|--facts] [--no-rows --no-summaries --no-entry --no-imports] [--value-depth N]
                                   [--search-path DIR]… [--follow PKG]… [--site-packages DIR]…
rowan/target/release/rowan serve                    # JSON lines over stdio (README has the protocol)
uv run --no-project python tests/run_golden.py      # 313/313 in ~1.1 s; paths select cases; -v shows reports
uv run --no-project python tests/run_snapshots.py   # SAME on 4 snapshots (fixed caps, no deadline); --update after review
cd vscode-rowan && node test.js && npm run package  # hover rendering check; rebuilds rowan-0.1.0.vsix
```

Env: `ROWAN_DEBUG=1` (decisions: which event tabulates a call, summary used, budget hits,
cache hits, effort line), `ROWAN_DEBUG=2` (plus every event, raise, guarded access, try
decision, unreachable statement), `ROWAN_NO=…` (switch one refinement off:
`recency,scc,site,narrowed,positional` options and the bisecting switches
`paramnarrow,partialclear,usermodel,dispatch1,inph,isinst,ddfactory,ddnarrow,boundtmpl,bindsub,ifexp,dcfield,split,builtincls,clonectx,sigsum`;
for the r–w rounds `globalrd,raisestate,chaincmp,counterread,range,extinst,whileunroll,matchpat,compcell,dunderdict,numkey,finally,yieldfrom,filteritems,zipn,cmhook,truthdunder,exhaust,getattrdunder,listcat,listinplace,iadd,excvalue,ntmethods,useriter,enummembers,initsubclass,clsattrpath,c3,descriptors,setattrdunder,dictsub,dunmissing,typeddictcall,keysetops,memo,nextend,kwreq,fmtkw,pctmap`),
`ROWAN_LIMITS=ms=2000,tab=256,call=256,clones=256,chain=2,path=3` (soft deadline, then hard
caps), `ROWAN_FOLLOW=pkg,…`, `ROWAN_SUMMARIES=0` (old per-call-site engine, comparison only),
`ROWAN_TODO=check` (harness checks `#@todo` expectations; none exist today).

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
| `report.rs` | dedupe, schema table, text / JSON lines / `--golden` / `--facts` (`text` = rendered value). |
| `main.rs` | CLI, `serve_loop`, `run_with_deps`, site-packages discovery; analysis on a 1 GiB-stack thread. |
| `rowan/py/` | PyO3 binding; the wheel does not load on this machine (see problems). |
| `tests/run_golden.py`, `tests/cases/` | 313 golden cases. Directives: `#@ tier key [; …]` (verdict on that line's access), `#@row NAME: present={..} definite={..} closed|open` (row of NAME on that line), `#@schema src@L<n>: always={..}`, `#@todo …` (reported, not checked), `#@argv …` (per-case flags, relative paths against the case dir). A package case is a directory whose entry shares its prefix (`k94_pkg/k94_modules.py`); other files there are modules. |
| `tests/run_snapshots.py`, `tests/snapshots/`, `tests/fixtures/` | `--json` snapshots on the example and three old-prototype modules (inputs only); the regression guard for internals. |
| `vscode-rowan/` | extension: talks to `rowan serve` (falls back to the CLI), hover = facts for (line, name), verdicts as diagnostics. |

Cases: `a01`–`e36` original design, `f37` recency, `g40`–`g65` AST coverage, `h66`–`h72`
objects passed around, `i73`–`i92` patterns, `j93` an 830-line system, `k94_pkg` flat
modules, `l95_pkg` a real package, `m96_pkg` a 1,700-line twelve-module warehouse (155
checks), `n97_pkg` site-packages, `o98`–`o103`, `p104`–`p105`, `q106`–`q135` and six rounds
of 30 more (`r136`–`r165`, `s166`–`s195`, `t196`–`t225`, `u226`–`u255`, `v256`–`v285`,
`w286`–`w315`, the last on input schemas) one-hypothesis cases. Every `h`+ case was written as a value specification *before* running the engine;
expectations that turned out wrong carry a comment saying why. Convention: a comparison or
arithmetic on known numbers / strings is decided; anything involving an unknown value keeps
both paths and is written as `?`; `json.loads` results are open by design.

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
* **Positions.** Literal containers, appends in straight-line code (a decided `if` counts as
  straight-line), `extend` with known items, comprehensions whose filters are decided,
  `sorted` with known literal keys (stable, `reverse=`), `reversed`, slices with known
  bounds, generator yields (with a `next()` cursor), `zip`/`enumerate`/`map` over known
  items keep positions; appends of unknown count keep the prefix (`known_prefix`); `sorted`
  with unknown keys keeps only the count. `insert`/`del xs[i]`/`pop`/`clear`/in-place
  `sort`/`reverse`, `a + b`, `a * n`, step slices, starred targets, `zip(*rows)` and
  `range` (positions up to 64, bounds decide `in`) keep them too; `xs += ys` extends in
  place. Dicts keep insertion order in `RowData.order` (`ordered_keys`): iteration,
  `popitem`, `move_to_end`. One-shot iterators (`Obj.iterator`) are exhausted by a full
  iteration; numeric keys `True`/`1`/`1.0` are one key (`canon_key`).
* **Values.** `+ - * // / %` and `< <= > >=` on literals, `len`, `sum`, `bool`, `min`/`max`,
  `%` and f-string formatting with widths, `str.split/partition/join/startswith/endswith`,
  membership in positional containers of literals, `and`/`or`/`not`/`IfExp` on decided
  truthiness, `x is None`, `==`, chained comparisons, substring `in`, `any`/`all`,
  `int`/`float` (a literal that is no number raises ValueError), `str.format`, string
  methods, `reduce`, `filter`; `while` iterations whose test is decided at the head are
  unrolled (constant tests like `while True` are not); `match` decides literal, singleton,
  or-, capture and mapping patterns; `k in d` on closed records can make a branch dead.
  `__bool__`/`__len__` decide truth at control flow (`truth_of`). A summary reads a module
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
* **Imports.** Local modules and packages (`__init__.py`, `import a.b.c` as attributes,
  relative imports via `ModuleInfo::pkg`, `from pkg import sub`), roots = file dir, the dir
  above its outermost package, `--search-path`; allowlisted installed packages
  (`--follow`) from site-packages unless a model covers them.
* **Budgets.** `MAX_MS = 2000`: past it no new tabulations and no new clones (results stay
  deterministic per machine speed, less precise on inputs that take longer); hard caps
  `MAX_TABULATIONS` / `MAX_CALL_TABULATIONS` = 256 per function, `MAX_CLONES` = 256 per site,
  `MAX_ACTIVE` = 4, `MAX_SUMMARY_ROUNDS` = 3, `UNROLL_MAX` = 16, `PATH_DEPTH` = 3.
* **Server.** `rowan serve` caches each analysed file's `--facts` result with the content
  hashes of every module it loaded and reuses it while none changed; parsed trees are
  cached per (path, hash). Summaries of unchanged modules are *not* reused across runs (they
  hold process-local ids); that is the next refinement if a project shows multi-second
  entries. Measured on `m96_pkg`: 160 ms first, 8 ms unchanged, 125 ms after an edit.

## Measured (release, Apple M-series)

| input | time | notes |
| --- | --- | --- |
| golden suite (313 cases) | 1.1 s | one process per case |
| `j93_order_system.py` (830 lines) / `m96_pkg` (1,716 lines) | 0.09 s / 0.10 s | all checks pass |
| `tests/fixtures/proto_summary.py` (1.2k lines, self-referential) | 2.5 s (hits the deadline) | 0 ✗, 28 ⚠ |
| old prototype `interp.py` (3.4k lines) | ~70 s before the deadline existed | 961 ✗, noisy (see problems) |

## Known problems and limits

* PyO3 wheel: builds but fails to load on macOS 26 / Xcode 26 with rustc 1.91 (dyld
  "mis-aligned LINKEDIT string pool"); CLI and extension do not use it.
* Large self-referential programs (the old prototype) are noisy: stores through a sibling
  closure's cell seen only via the factory's summary, dispatch over `self.objects` tables.
  Precision on real repositories (spec §15.4 runtime oracle) was never measured; only
  robustness (137 local scripts, no failures).
* A store through a name holding several objects is weak on each (golden `h72`, `maybe`).
* An unknown call whose arguments hold records may alias them; its attribute row is open.
* Mutual recursion widens after 3 rounds; `while` loops with unknown conditions join;
  comparisons on unknown values keep both branches; `match` class / sequence patterns stay
  undecided; `getattr` needs a known name; pandas columns from data stay open.
* `dict(zip(keys, row))` with a row of unknown length makes every key definite (design
  case `d30`); with both lengths known zip stops at the shorter. A `for` over a one-shot
  iterator exhausts it even when the loop breaks. `ChainMap` is a merged copy (stores do
  not reach the first map). `lru_cache` results are flow-insensitive.
* Recency's flow-insensitive side tables other than `cls_slots` are not renamed (stale
  cache entries only waste memory). `Row::concretise` resolves referenced entry rows
  transitively. ASTs are leaked per file version.
* Snapshots on the big fixture depend on the fixed caps the snapshot harness pins
  (`ROWAN_LIMITS=ms=100000000,tab=24,call=32,clones=8`); the default run of that fixture
  degrades after 2 s.

## Where to go next

The agreed plan (values in notation, cross-module, specialisation, packages, the 2,000-line
example, server, site-packages, more examples) is complete; each step was one commit. Open
items, in the order they seem worth doing: (1) summary reuse across runs in the server;
(2) the weak store through several objects (path-sensitive ownership or a narrowed row on
the name); (3) precision on self-referential programs and a runtime oracle on real repos;
(4) more one-hypothesis cases — writing expectations *before* running the engine found 75
engine bugs in the six rounds r–w (21, 13, 22, 12, 2, 5): library and protocol models
dominate; the core flow / aliasing round (v) found two. Keep this file under 200 lines.
