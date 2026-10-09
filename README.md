# rowan — an abstract interpreter for Python

`rowan` reads a Python program (plus local imports) and interprets it abstractly: every
name holds a set of possible values — literals, objects (dicts, lists, instances,
functions, closures, iterators, exceptions, …) or `?` for a value it cannot know. From
that it reports what each name holds at each point, the key and attribute accesses that
can fail (definitely, possibly, or only if unknown code interfered), and what the program
demands of values that enter from outside (`read_csv`, `json.load`, request bodies,
`os.environ`, parameters of functions nobody calls): the inferred schemas.

It started as key-set inference for dict-like records; the key sets of dicts and instance
attributes are now one model among others (list positions, literal arithmetic, string
operations, iterators, the standard library, pandas, and PyTorch tensor shapes). Functions with
no callers are analysed with open parameters, so their variables get results too.

**PyTorch.** In a program that imports torch, every tensor has a symbolic shape in the terms
of the function it is in: `h = self.fc(x)` inside `forward` is `Tensor[*x.shape[:-1], 4*self.n_embd]`,
`x @ y` on two parameters is `Tensor[*broadcast(x.shape[:-2], y.shape[:-2]), x.shape[-2],
y.shape[-1]]` (if both have at least 2 dims). Layers and functions check their conditions: a
definite mismatch (`Linear(512 -> 10) given last dim 256`, an RNN given 4 dims) is reported;
an undecided one (`x.shape[-1] == self.n_embd`) is a *requirement* shown at the line and
listed under `== tensor shapes ==`, and holds from there on. The hover names dims by the
locals that hold them (`B, T, C = x.shape` → `Tensor[B, T, 3*self.n_embd]`). The design is in
`SPECIFICATION.md` §2.3 and `tests/torch/CONVENTIONS.md`; the 162 torch test programs (145 small, 17
well-known models from a from-scratch transformer to nanoGPT, ResNet, U-Net, ViT, BERT) are
in `tests/torch/`, each checked against real PyTorch by `tests/torch_oracle.py`.

**pandas.** A frame's key set is its columns; beside it rowan keeps what literal data tells
(index levels, row labels and count, each column's literal values and dtype), so a
`pivot_table` / `crosstab` / `get_dummies` over known values has known columns, `describe()` the
numeric ones, `reset_index()` the index names; a Series whose index matters (a column, a row,
`df.sum()`, a grouped aggregate) is a record of its labels. Merges decide each column by which
side may have it (an open input never provides a plain name the other side surely has), and
calls inside functions are analysed per call site. 115 test programs (100 small, 15
pipelines) in `tests/pandas/` are checked against real pandas by the runtime oracle
`tests/oracle.py`, which also checks every claim of the 417 plain-Python cases that runs.

The design is in `SPECIFICATION.md`; `AGENTS.md` has the engineering notes for whoever
works on it next. The engine is written in Rust (`rowan/`); it was ported from a Python
prototype that has since been removed (see git history before `a1f4d7a` if you need it).

```
cd rowan && cargo build --release                 # the engine: rowan/target/release/rowan
rowan/target/release/rowan prog.py                # text report
rowan/target/release/rowan prog.py --json         # JSON lines (verdicts, schemas, summaries)
rowan/target/release/rowan prog.py --facts        # per-name facts for editor integration
rowan/target/release/rowan lib.py --no-entry      # only what the module itself runs (no entry-point pass)
uv run --no-project python tests/run_golden.py    # golden suite (417 cases + 162 torch + 115 pandas)
uv run --no-project --with pandas python tests/oracle.py   # the runtime oracle on the cases (strict on tests/pandas)
uv run --no-project python tests/run_snapshots.py # output snapshots on larger inputs
```

`rowan serve` keeps one process alive and speaks JSON lines over stdio:
`{"id":1,"cmd":"analyse","path":"/abs/file.py"}` answers with the `--facts` document under
`result` (plus `cached`, `ms` and `deps`, the modules it loaded); the result is reused while
the file and every module it loaded are unchanged (content hashes), `{"cmd":"changed",
"paths":[…]}` drops the results that depend on those files, `{"cmd":"shutdown"}` exits.
Parsed modules are kept per (path, content hash). The VS Code extension uses it by default
(`rowan.useServer`).

Options: `--no-rows`, `--no-summaries`, `--no-entry`, `--no-imports`, `--value-depth N`
(nesting depth of rendered values, default 3), `--search-path DIR`
(repeatable; the file's directory and the directory above its outermost package are
searched first), `--follow PKG` (repeatable or comma-separated, also `ROWAN_FOLLOW`): analyse
that installed package's source from site-packages (`--site-packages DIR`, else
`$VIRTUAL_ENV` or a `.venv` / `venv` next to the file or an ancestor; no interpreter is
run); a package with a registered model (pandas, numpy, …) keeps the model. Imports are
followed into local modules and packages (`pkg/__init__.py`, `import a.b.c`, relative
imports); other installed packages are modelled or unknown. Environment:
`ROWAN_SUMMARIES=0` (older per-call-site engine), `ROWAN_DEBUG=1` (progress, which event
makes a call site re-analyse, slow instantiations on stderr), `ROWAN_NO=…` (switch one
analysis refinement off, for bisecting; names in `AGENTS.md`),
`ROWAN_LIMITS=ms=2000,tab=256,call=256,clones=256,chain=2,path=3` (the soft time limit after
which the analysis stops refining, then the hard per-function budgets and depths). Python tooling runs through `uv`
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

Values are written as Python-like text, as far as they are known (spec §2.2):

| Shown | Meaning |
| --- | --- |
| `?` | a value rowan does not know (any type) |
| `a`, `a(b)`, `cfg["db"]`, `rows[*]`, `json.loads(text)`, `1 + b` | an unknown named in the program's terms (a parameter, the result of an unknown call, a read from another unknown, an input, an expression); shown only inside the function whose names it uses, `?` elsewhere; the hover's legend says what is known about each (`a: ?`) |
| `1 \| 2 \| ?` | one of several possibilities |
| `{"id": 1, "role"?: "admin", "x"~: ?}` | a dict with exactly these keys: `"id"` always, `"role"` on some paths only, `"x"` unless unknown code removed it |
| `{"id": 1, **?}` | the same, plus possibly other keys (an unknown dict spread in) |
| `Order(id=1, **?)` | an instance and its attributes, same marks |
| `[1, ?]`, `(1,)` | a list / tuple with known positions |
| `list[X]`, `set[X]`, `tuple[X, ...]` | any number of elements, each like `X` |
| `{…}`, `[…]`, `Name(…)`, `…` | left out by the printer (nesting depth, more than eight entries, a cycle) |
| `Tensor[B, T, 4*C]`, `Tensor[*x.shape[:-1], 10]`, `Tensor[*, 10]` | a tensor of that shape (dims named in the legend; `*`: unknown leading dims) |

The hover in the VS Code extension, the `module-level values` section of the text report
and the `text` field of `--facts` use this form; `--json` and `--facts` also carry the
key-set model's underlying sets (`present`, `definite`, `likely`, `absent`, `closed`,
`sources`, `mutators`).

```
== diagnostics ==
  L61   ✗     r['totl']        item['totl'] definitely missing  (all keys known)  [row@L59]  via L96>L85
  L70   ✗     row['count']     item['count'] definitely missing  (all keys known)  [vars@L67]
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
definitely missing (all keys known, deleted, renamed away, suffixed by a merge, projected
out), `⚠~` likely missing (only unknown code could have added it), `◌` depends on
input (becomes a schema entry; for attributes not listed as a diagnostic), `⚠data` data-derived key into a dict whose keys are all
known.
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
| `rowan/src/models/` | library models: builtins, `dict`/`list`/`str`, `json`/`yaml`/`pickle`, `copy`, `dataclasses`, `collections`, `csv.DictReader`, `argparse`, `os.environ`, `functools.partial`, `requests`, pandas, torch (`torch.rs`) |
| `rowan/src/shape.rs` | tensor shapes: symbolic dims, shapes, facts, printing |
| `rowan/src/report.rs` | per-access verdict combination, schema table, text / JSON / facts output |
| `tests/run_golden.py`, `tests/cases/` | golden harness: expectations are `#@` comments in the case files (417 cases, each written before running the engine) |
| `tests/torch/`, `tests/torch_oracle.py` | 162 torch cases (`CONVENTIONS.md`); the oracle runs them under real PyTorch |
| `tests/pandas/`, `tests/oracle.py` | 115 pandas cases (`CONVENTIONS.md`, `PENDING`); the oracle runs every case under real pandas / Python (rows, values, verdicts, schemas by mutation) |
| `tests/run_snapshots.py`, `tests/snapshots/`, `tests/fixtures/` | output snapshots on the example and three large fixture programs |
| `examples/etl_report.py` | a realistic script with planted bugs (all found; also golden case e34) |
| `vscode-rowan/`, `rowan/py/` | VS Code extension (uses `rowan serve`); PyO3 binding (does not load on the current macOS toolchain) |

## Deviations from the spec

* **Symbolic summaries, with concrete fallback.** A function body is analysed once
  against placeholders (its parameters, `self`, and lazily every pre-existing object it
  touches); the result is instantiated at each call site (spec §7.3/§9). A call site is
  analysed concretely instead only when it has information an *imprecision event* of the
  body is about — a call or method dispatch through a parameter that is an actual object
  here, `isinstance`/truthiness/`in` on a parameter that is a literal here, iteration
  over a parameter that is a literal container here, arithmetic or formatting on a
  parameter, a forwarded `*args` / `**kwargs` — within a 2 s soft deadline and hard caps
  (256 per function). Summaries are specialised by the classes of the receiver and of
  instance arguments, and by the closure cells they see. Recursion inside a summary
  iterates to a fixpoint over at most 3 rounds; summaries computed against a partial
  summary are redone per round.
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

Release build, Apple M-series: the 579-case golden suite runs in 2 s (one process per
case; each torch model in about 0.02 s); an 830-line program and a 1,700-line twelve-module package analyse in 0.1 s each;
a 1.2k-line self-referential module hits the 2 s soft deadline and finishes in 2.5 s with
reduced precision. 137 scripts from local projects analysed in 0.8 s total without
failures. Comparisons and arithmetic on known numbers, positions through comprehensions,
sorting and generators, and string formatting are evaluated, so small programs usually get
exact rows.

## Known limitations

See SPECIFICATION.md §16 and the known-problems section of AGENTS.md.
