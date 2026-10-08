# rowan — static key-set inference for Python records (Phase-0 prototype)

`rowan` reads a Python program (plus local imports) and infers, for every dict-like
value — dicts, DataFrame columns, object attributes, TypedDicts, `**kwargs`, JSON,
CSV rows, argparse namespaces, `os.environ` — which keys it has at each point and
which keys the program *demands* of it. Demands on values that enter from outside
(`read_csv`, `json.load`, request bodies, entry-point parameters) are reported as
inferred schemas.

The design is in `SPECIFICATION.md`. This directory is the Phase-0 Python prototype
(§15.1 of the spec): it validates the domain, the models and the heap treatment; it
is not the fast engine (§15.3).

```
python -m rowan path/to/program.py            # text report
python -m rowan path/to/program.py --json     # JSON lines (verdicts, schemas, summaries)
python -m rowan lib.py --no-entry             # only what the module itself runs (no entry-point pass)
python tests/run_golden.py [-v] [case.py …]   # golden suite (36 cases)
ROWAN_SUMMARIES=0 python -m rowan prog.py     # the older per-call-site engine (for comparison)
ROWAN_DEBUG=1 python -m rowan prog.py         # progress / summary statistics on stderr
```

## Rust engine (`rowan-rs/`)

The engine has been ported to Rust (spec §15.3); the Python package above is kept as the
frozen oracle. It parses with `ruff_python_parser`, interns keys / sources / lenses to
`u32` ids, and reproduces the prototype's verdicts and schemas exactly in `--compat` mode
(`python tests/diff_engines.py` checks that on every golden case, the example and the
prototype's own modules). Its default mode adds the improvements listed in `AGENTS.md`
(allocation-site resolution of placeholders, recency abstraction for loop allocations,
SCC re-iteration for mutual recursion, `narrowed` facts in state equality, positional
unpacking of tuple keys).

```
cd rowan-rs && cargo build --release
rowan-rs/target/release/rowan-rs prog.py [--json] [--no-rows] [--no-summaries] [--no-entry] [--no-imports] [--compat]
uv run --no-project python tests/run_golden.py --rust    # golden suite on the Rust engine (63 cases)
uv run --no-project python tests/run_golden.py --both    # both engines (the 36 prototype cases run on both)
uv run --no-project python tests/diff_engines.py         # differential check: Python oracle vs rowan-rs --compat
```

`ROWAN_SUMMARIES=0`, `ROWAN_DEBUG=1` work as for the prototype; `ROWAN_COMPAT=1` is
`--compat`; `ROWAN_NO=recency,scc,site,narrowed,positional,fixes` switches improvements off
individually (`fixes` is a set of semantic corrections over the prototype found by the
extended golden suite; `AGENTS.md` lists them). Python is run through `uv`
(`.python-version` = 3.14). `rowan-rs/py/` holds a PyO3 binding (`rowan_rs.analyse_file(path)` returns
the `--golden` JSON); see `AGENTS.md` for its status.

## VS Code extension (`vscode-rowan/`)

Hover over a variable to see the keys / attributes rowan infers for its objects (always,
on some paths, known absent, open sources), the keys the program requires of an external
input, possible literal values, and the verdicts on that line; verdicts also appear as
diagnostics. It runs `rowan-rs FILE --facts` on open and save. Build the engine, then
install `vscode-rowan/rowan-0.1.0.vsix` (built with `npm run package` in that folder; `Extensions: Install from VSIX…`) or open the
folder in VS Code and press F5; see `vscode-rowan/README.md`.

## What the report contains

```
== diagnostics ==
  L61   ✗     r['totl']        item['totl'] definitely missing  (closed row)  [row@L59]  via L96>L85
  L70   ✗     row['count']     item['count'] definitely missing  (closed row)  [vars@L67]
  L88   ◌     cfg['verbose']   item['verbose'] depends on input  ← json.load@L28  [dict@L25]
  L11   ✓~    opts['host']     ok (likely — unknown code ran in between)
  L12   ⚠~    opts['user']     likely missing  (only lib.apply_defaults@L10 could have added it)
  L8    ⚠     t['l']['r']      possibly missing  (depends on which object: ✗ for dict@L3; ✓ for dict@L4)

== inferred schemas (requirements on external inputs) ==
  read_csv@L83:
      always   : 'amount' | 'amt', 'cust' | 'customer_id', 'order_id'
  csv.DictReader@L36:
      always   : 'customer_id', 'name', 'tier'
  json.load@L28:
      always   : 'verbose'
```

Tiers (spec §3.1): `✓` proven, `✓~` likely (optimistic: unknown code intervened),
`⚠` possibly missing (set on some paths only / depends on which object), `✗`
definitely missing (closed row, deleted, renamed away, suffixed by a merge, projected
out), `⚠~` likely missing (only unknown code could have added it), `◌` depends on
input (becomes a schema entry), `⚠data` data-derived key into a closed record.
`via L96>L85` is the call chain the access ran under: verdicts are per call context.

## Layout

| file | what |
| --- | --- |
| `rowan/domain.py` | the abstract domains: `Row` (present/definite/likely/absent, open-source lenses, mutators), `Lens` steps (rename / kill) with inverse images for requirement propagation, `KeyVal` (Lit / Lits / KeysOf / Data / ⊥), `ReqInfo`, `Value`, `classify()` |
| `rowan/state.py`, `rowan/layered.py` | abstract objects and the flow-sensitive state (env, rows, slots, container elements, requirements, return accumulator); `layered.py` holds the persistent layered maps (O(1) copy, O(diff) join/equality/footprint) |
| `rowan/scopes.py` | lexical scopes: locals, cells captured by closures, free variables |
| `rowan/interp.py` | the abstract interpreter over the AST: statements, expressions, calls (symbolic summaries, with per-call-site analysis as the fallback), closures as cell objects, classes / MRO / properties / `__getitem__`, declared field classes, loops (literal unrolling, exhaustive key iteration), narrowing (`in`, `.get`, `hasattr`, `isinstance` incl. attribute chains, `all(k in d …)`, `S <= d.keys()`, `k in d` for a key that is one of several literals), guarded loads in `try`, unknown calls with attribution, entry-point analysis of uncalled functions |
| `rowan/summary.py` | symbolic function summaries (spec §7.3/§9): placeholders, symbolic state, deferred accesses (`SymReq`), imprecision events, footprint computation (diff + reachability), instantiation |
| `rowan/models.py` | library models: builtins, `dict`/`list`/`str` methods, `json`/`yaml`/`pickle`, `copy`, `dataclasses`, `collections`, `csv.DictReader`, `argparse`, `os.environ`, `functools.partial`, `requests`, pandas (read_*, `[]`/`loc`, `assign`, `rename` incl. callables, `drop`, `merge`/`join` with suffixing, `concat`, `groupby.agg`, `set_index`/`reset_index` with index tracking, `apply(axis=1)`, `iterrows`, `to_dict`, `melt`, `add_prefix`, `columns = …`) |
| `rowan/report.py` | per-access verdict combination across points-to targets, schema table, text / JSON output |
| `tests/run_golden.py`, `tests/cases/` | golden harness: expectations are `#@` comments in the case files |
| `examples/etl_report.py` | a realistic script with planted bugs (all found) |

## Deviations from the spec in this prototype

* **Symbolic summaries, with concrete fallback.** A function body is analysed once
  against placeholders (its parameters, `self`, and lazily every pre-existing object it
  touches); the result is instantiated at each call site (spec §7.3/§9). A call site is
  analysed concretely instead only when it has information an *imprecision event* of the
  body is about — a call or method dispatch through a parameter that is an actual object
  here, `isinstance`/truthiness/`in` on a parameter that is a literal here, iteration
  over a parameter that is a literal container here — and, for the literal/structural
  events, at most `MAX_TABULATIONS` (12) times per function. Recursion inside a summary
  iterates to a fixpoint over at most 6 rounds; mutual recursion through a function
  already being summarised uses that function's current partial summary (the callee's
  summary is then cached as final — a known imprecision, spec §9.6).
* **What a summary keeps** is the exit/entry *difference* of the heap, minus objects the
  caller can never reach; deferred accesses carry one call site of context inside the
  summary (the caller's concrete chain is prepended on use); closure summaries are
  specialised on the code their cells hold.
* **Declared types are trusted** for narrowing (§8.5): a parameter annotated with a
  user class, a field assigned `C(...)` in the class body, a dataclass field `x: C` give a
  placeholder its class, which resolves methods and filters the objects it can stand for;
  a field annotated `Any`/`object` makes accesses through it unreportable (the report
  drops an access when one of its possible targets is unknowable).
* **Requirement levels are forward-accumulated**: `always` means "on every path from
  program start", joins demote to `sometimes`; requirements on a loop's own elements
  (reader rows, frame rows) are kept `always` since they are per element.
* **Disjunctions** are reported as alternatives (`'email': or provided by …`,
  `'amount' | 'amt'`) rather than by demoting to `sometimes`.
* **Performance.** The heap parts of a state live in layered persistent maps
  (`rowan/layered.py`): copying a state is O(1) and joining two states costs time
  proportional to what changed since they were forked. Measured on this analyser's own
  modules (entry-point pass included, imports followed; "✗" counts definite-missing
  lines in the report, all of which were false positives):

  | module (lines incl. imports) | per-call-site engine (`ROWAN_SUMMARIES=0`) | symbolic summaries |
  | --- | --- | --- |
  | 36-case golden suite | 0.3 s | 0.3 s |
  | `scopes.py`, `layered.py`, `domain.py` (0.2–0.9k) | 0.2–0.6 s | 0.2–0.7 s |
  | `report.py`, `state.py` (~1k) | 1 s / 28 s, 169 ✗ | 1.3 s / 1.0 s, 0 ✗ |
  | `summary.py` (2.6k) | — | 29 s, 0 ✗, 260 ⚠ |
  | `models.py` (7.6k: its entry points reach the whole interpreter) | 23 s, 11 ✗ | not completed in 50 min (see below) |
  | `interp.py` (7.6k, 380 roots) | minutes (not completed) | not completed (see below) |

  The Rust engine on the same inputs (release build, Apple M-series; `--compat` is the
  prototype's algorithm, "default" adds the improvements):

  | module | rowan-rs `--compat` | rowan-rs default |
  | --- | --- | --- |
  | 37-case golden suite | 0.4 s (one process per case) | 0.4 s |
  | `scopes.py` … `state.py` | 0.00–0.02 s, 0 ✗ | 0.00–0.02 s, 0 ✗ |
  | `summary.py` | 0.6 s, 0 ✗ | 0.8 s, 0 ✗ |
  | `models.py` | 154 s, 126 ✗ | 41 s, 165 ✗ (same accesses, more contexts) |
  | `interp.py` | 540 s, 1039 ✗ | 69 s, 961 ✗ |

  The two heavy rows are the analyser analysing its own interpreter: `models.py`'s
  functions take a `Call` whose `an: Analyzer` field now has a declared class, so the
  symbolic analysis follows `an.iterate(...)`, `an.call_value(...)` into the
  interpreter's mutually recursive core (the per-call-site engine saw an unknown object
  there and gave up, cheaply and imprecisely). Summarising that core is where the time
  goes: its functions are instantiated thousands of times while the summaries of their
  callers are built, and in the abstract heap of a 7.6k-line program run under one
  `Analyzer` every placeholder stands for dozens of objects, so each deferred access is
  evaluated against dozens of rows. An intermediate version of this engine finished
  `models.py` in ~100 s with 0 ✗; the precision work after it (declared field classes,
  `isinstance` narrowing on parameters and attribute chains, strong constructor stores)
  made these summaries larger and the run no longer completes in 50 minutes. Everything
  measured on ordinary modules got faster and reported no false ✗. `--no-entry` analyses
  only what a module runs itself (`interp.py`: 0.6 s), and `ROWAN_SUMMARIES=0` selects
  the per-call-site engine.

## Known limitations

See SPECIFICATION.md §16. Notably: keys from arbitrary string computation, `getattr`
with computed names, `match` statements (bodies analysed, no narrowing), pandas
operations whose columns come from data, Series index semantics, `__getattr__`
magic, and the optimistic tiers (`✓~`, `⚠~`) are deliberately unsound.
