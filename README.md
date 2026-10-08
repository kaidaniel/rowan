# rowan — static key-set inference for Python records

`rowan` reads a Python program (plus local imports) and infers, for every dict-like
value — dicts, DataFrame columns, object attributes, TypedDicts, `**kwargs`, JSON,
CSV rows, argparse namespaces, `os.environ` — which keys it has at each point and
which keys the program *demands* of it. Demands on values that enter from outside
(`read_csv`, `json.load`, request bodies, entry-point parameters) are reported as
inferred schemas.

The design is in `SPECIFICATION.md`; `AGENTS.md` has the engineering notes for whoever
works on it next. The engine is written in Rust (`rowan/`); it was ported from a Python
prototype that has since been removed (see git history before `a1f4d7a` if you need it).

```
cd rowan && cargo build --release                 # the engine: rowan/target/release/rowan
rowan/target/release/rowan prog.py                # text report
rowan/target/release/rowan prog.py --json         # JSON lines (verdicts, schemas, summaries)
rowan/target/release/rowan prog.py --facts        # per-name facts for editor integration
rowan/target/release/rowan lib.py --no-entry      # only what the module itself runs (no entry-point pass)
uv run --no-project python tests/run_golden.py    # golden suite (90 cases)
uv run --no-project python tests/run_snapshots.py # output snapshots on larger inputs
```

Options: `--no-rows`, `--no-summaries`, `--no-entry`, `--no-imports`. Environment:
`ROWAN_SUMMARIES=0` (older per-call-site engine), `ROWAN_DEBUG=1` (progress and slow
instantiations on stderr), `ROWAN_NO=recency,scc,site,narrowed,positional` (switch one
analysis refinement off, for bisecting). Python tooling runs through `uv`
(`.python-version` = 3.14); the Python side is only the test harness.

## VS Code extension (`vscode-rowan/`)

Hover over a variable to see the keys / attributes rowan infers for its objects (always,
on some paths, known absent, open sources), the keys the program requires of an external
input, possible literal values, and the verdicts on that line; verdicts also appear as
diagnostics. It runs `rowan FILE --facts` on open and save. Build the engine, then
install `vscode-rowan/rowan-0.1.0.vsix` (built with `npm run package` in that folder;
`Extensions: Install from VSIX…`) or open the folder in VS Code and press F5; see
`vscode-rowan/README.md`.

A PyO3 binding (`rowan/py/`, `rowan.analyse_file(path)` → the `--golden` JSON) builds with
maturin; see `AGENTS.md` for its status.

## What the report contains

Records are written in a compact form: `{"id": ..., "role"?: ..., "tag"~: ..., **}` is a
dict that always has `"id"`, has `"role"` on some paths only (`?`), had `"tag"` unless
unknown code removed it (`~`), and may hold other keys (`**` = open; without it the record
is closed and these are all its keys). Objects use the same notation with attributes:
`obj(name=..., out?=..., **)`. The hover in the VS Code extension and the
`module-level records` section of the text report use this form; `--json` and `--facts`
also carry the underlying sets (`present`, `definite`, `likely`, `absent`, `closed`,
`sources`, `mutators`).

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

| path | what |
| --- | --- |
| `rowan/src/intern.rs` | interners (keys, strings, sources, lenses → `u32`), sorted id-sets |
| `rowan/src/domain.rs` | the abstract domains: `Row` (present/definite/likely/absent, open-source lenses, mutators), `Lens` steps with inverse images, `KeyVal`, `ReqInfo`, `Value`, `classify()` |
| `rowan/src/state.rs`, `rowan/src/layered.rs` | abstract objects and the flow-sensitive state; persistent layered maps (O(1) copy, O(diff) join/equality/footprint) |
| `rowan/src/scopes.rs`, `rowan/src/frontend.rs` | lexical scopes; parsing (ruff), node locations |
| `rowan/src/interp/` | the abstract interpreter over the AST: statements, expressions, calls, closures as cell objects, classes / MRO / properties / dunder protocols, declared types, loops with recency, narrowing, guarded loads, unknown calls with attribution, entry-point analysis of uncalled functions |
| `rowan/src/summary.rs` | symbolic function summaries (spec §7.3/§9): placeholders, deferred accesses, imprecision events, footprints, instantiation with allocation-site resolution, SCC re-iteration |
| `rowan/src/models/` | library models: builtins, `dict`/`list`/`str`, `json`/`yaml`/`pickle`, `copy`, `dataclasses`, `collections`, `csv.DictReader`, `argparse`, `os.environ`, `functools.partial`, `requests`, pandas |
| `rowan/src/report.rs` | per-access verdict combination, schema table, text / JSON / facts output |
| `tests/run_golden.py`, `tests/cases/` | golden harness: expectations are `#@` comments in the case files (90 cases) |
| `tests/run_snapshots.py`, `tests/snapshots/`, `tests/fixtures/` | output snapshots on the example and three large fixture programs |
| `examples/etl_report.py` | a realistic script with planted bugs (all found; also golden case e34) |
| `vscode-rowan/`, `rowan/py/` | VS Code extension; PyO3 binding |

## Deviations from the spec

* **Symbolic summaries, with concrete fallback.** A function body is analysed once
  against placeholders (its parameters, `self`, and lazily every pre-existing object it
  touches); the result is instantiated at each call site (spec §7.3/§9). A call site is
  analysed concretely instead only when it has information an *imprecision event* of the
  body is about — a call or method dispatch through a parameter that is an actual object
  here, `isinstance`/truthiness/`in` on a parameter that is a literal here, iteration
  over a parameter that is a literal container here — and, for the literal/structural
  events, at most 12 times per function. Recursion inside a summary iterates to a fixpoint
  over at most 3 rounds; summaries computed against a partial summary are redone per round.
* **What a summary keeps** is the exit/entry *difference* of the heap, minus objects the
  caller can never reach; deferred accesses carry one call site of context inside the
  summary (the caller's concrete chain is prepended on use); closure summaries are
  specialised on the code their cells hold.
* **Declared types are trusted** for narrowing (§8.5): a parameter annotated with a
  user class, a field assigned `C(...)` in the class body, a dataclass field `x: C` give a
  placeholder its class, which resolves methods and filters the objects it can stand for;
  a field annotated `Any`/`object` makes accesses through them unreportable.
* **A successful access narrows the key in**: after `r["x"]` the key is assumed present
  on that path (the program went on). `.get`/`.setdefault` do not record requirements.
* **Loops**: literal iterables are unrolled (also with `break`/`continue`); otherwise a
  fixpoint over at most 40 iterations, with a recency abstraction (the object allocated by
  the current iteration is strong) and the iterable's element objects keeping the body's
  exit row after a loop without `break`.
* **Requirement levels are forward-accumulated**: `always` means "on every path from
  program start", joins and conditions demote to `sometimes`; requirements on a loop's own
  elements stay `always` since they are per element.
* **Disjunctions** are reported as alternatives (`'email': or provided by …`,
  `'amount' | 'amt'`) rather than by demoting to `sometimes`.

## Performance

Release build, Apple M-series: the 63-case golden suite runs in under a second (one
process per case); a 1.2k-line module analyses in about a second, 1.7k lines (`models.py`
of the old prototype, which reached a whole interpreter through a typed field) in about
40 s, 3.4k lines (`interp.py`) in about 70 s. 137 scripts from local projects analysed in
0.8 s total without failures.

## Known limitations

See SPECIFICATION.md §16 and the known-problems section of AGENTS.md.
