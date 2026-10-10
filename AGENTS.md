# AGENTS.md — rowan, handoff notes

An abstract interpreter for Python: every name holds a set of possible values (literals,
objects, `?` unknown); reports what names hold, accesses that can fail, and the schemas the
program demands of its inputs. Key sets of dicts / instance attributes / model fields (`Row`)
are one model among others; tensor shapes (PyTorch) are another. Design: `SPECIFICATION.md`
(§2 values and models, §2.3 tensors, §11 library models; §9.8 and §16 note where the engine
went further). User-facing: `README.md`. This file is for whoever picks the work up; true at
the last commit. `PLAN.md` is the running plan of the current request (pydantic, FastAPI,
stdlib, mixed programs, numpy).

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (`.python-version` pins 3.14); never
  the system or a local interpreter. The Python side is only the test harnesses.
* New test cases are *specifications*: write the expected rows and verdicts by hand from
  the program text first, then make the engine produce them. Never derive an expectation
  from engine output; when one turns out wrong on re-reading, fix it with a comment saying
  why (same line count: schema directives name lines). Value analysis (which keys, which
  columns) matters more than bug-finding verdicts. Soundness first.
* Commit without being asked whenever a round of bug fixes or a feature is complete and the
  tests pass (golden, snapshots, `cargo test`), one commit per round / step; never commit a
  failing state; review snapshot diffs before `--update`. Commit messages end with a
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
uv run --no-project python tests/run_golden.py      # 1015/1015 in ~5 s (all suites); paths select; -v
uv run --no-project python tests/run_golden.py --pending [--prune]   # cases in tests/*/PENDING (none left)
uv run --no-project --with pandas --with pydantic --with pydantic-settings --with fastapi --with httpx --with numpy \
    python tests/oracle.py [--strict|--lenient] [-v] [case…]         # expectations vs a real run (849/849, ~4 min)
uv run --no-project --with torch --with numpy python tests/torch_oracle.py  # torch expectations vs real PyTorch
uv run --no-project python tests/run_snapshots.py   # SAME on 4 snapshots (fixed caps, no deadline); --update after review
cd vscode-rowan && node test.js && npm run package  # hover rendering check; rebuilds rowan-0.1.0.vsix
```

Env: `ROWAN_DEBUG=1|2` (tabulation reasons, summaries, budgets; 2: every event, raise, try
decision), `ROWAN_NO=a,b` (switch refinements off: `rg 'disabled\("' rowan/src`),
`ROWAN_LIMITS=ms=2000,tab=256,call=256,clones=256,chain=2,path=3`, `ROWAN_FOLLOW=pkg,…`,
`ROWAN_SUMMARIES=0` (old per-call-site engine), `ROWAN_TODO=check` (check `#@todo`s too).

## Layout (Rust engine `rowan/`, ~58k lines)

| file | role |
| --- | --- |
| `intern.rs` | interners; keys incl. `Member` (Enum members, identity / frozen-dataclass value keys), `Frozen` (frozensets), `Dec` / `Frac` (Decimal / Fraction), `Chrono` (datetime values); `IdSet<T>`. |
| `domain.rs` | `Row` (present/definite/likely/absent, sources, mutators, terms), `KeyVal` (Lit/Lits/KeysOf/Data/Bot/Param), `Value`, `classify`. |
| `layered.rs`, `state.rs` | persistent layered maps; `State` (env, rows, slots, elems, `lens` (list length / known prefix per path), `lasts` (last appended), reqs, ret, exits). |
| `frontend.rs`, `scopes.rs` | ruff parsing, leaked `&'static` trees, node→module registry, parse cache, `site_prefix`; lexical scopes. |
| `interp/mod.rs` | `Analyzer` (+ model side tables: enums, argparse, csv, futures, singledispatch, identity keys, cookie jars…), `Obj`, allocation (`alloc_full`), `llen`/`set_llen`, `copy_side_tables`, budgets. |
| `interp/{stmts,exprs,funcs,narrow,access,calls}.rs` | statements, expressions, functions / iteration, narrowing, record / attribute access, calls (tabulation vs summary, call cache, dataclass init, singledispatch, exception objects). |
| `summary.rs` | placeholders, events, `summarise_sig`, `Inst` (instantiation), `propagate_events`, `should_tabulate`. |
| `models/mod.rs` | `Registry`; builtins, dict/list/set/str methods, `copy`, `dataclasses`, `collections`, `json` (exact `dumps` text, round trips, `object_hook`), `slice`, enum hooks. |
| `models/{pyvalues,pure,pytext}.rs` | exact CPython values on literals (str methods, format specs, `%`, math / operator / statistics; bytes methods, base64 / binascii, shlex, html, string.capwords, fnmatch, urllib.parse quoting; unit-tested against CPython). |
| `models/{counter,csvio,argparse,concurrency}.rs` | Counter / deque; `io.StringIO` text, `csv` writer / reader / Dict*, ExitStack, redirect_stdout, closing / nullcontext, the `stdobj.*` dispatch; argparse (parse of known argv); threads, executors, futures. |
| `models/{itertools,stdfuncs,configparser,sqlite}.rs` | itertools; heapq / bisect / operator / string.Template; configparser (INI simulated); sqlite3 (SQL simulated on in-memory DBs: expressions, aggregates, GROUP BY, ORDER BY alias / rowid, UPDATE SET expressions, rowcount; SELECT columns = row keys even when rows are not simulated; an outside connection met in a summary is left alone, `sql_touched`). |
| `models/{pyre,remodel,pynum,chrono,pathtext}.rs` | a Python-`re` backtracking engine and the `re` model (Pattern / Match objects); exact Decimal / Fraction; datetime / date / time / timedelta; os.path, pathlib pure paths, textwrap. |
| `models/{pydantic,fastapi}.rs` | pydantic v2 (+ v1 spellings) models, validation (Enum / Literal fields, constraints; defaults not validated), dumps, TypeAdapter, settings; FastAPI routes, binding (Optional scalars), dependencies (yield), TestClient dispatch, status codes, middlewares; responses as the client reads them (`json_wire`). |
| `models/{pandas,minire,torch}.rs`, `shape.rs` | pandas; a small regex matcher (pandas `regex=`); tensor shapes and the torch model. |
| `report.rs`, `main.rs` | output formats; CLI, `serve_loop`, site-packages discovery (1 GiB-stack thread). |
| `tests/run_golden.py`, `tests/oracle.py` | golden harness (directives: `tests/CONVENTIONS.md`), runtime oracle (strict on the library suites). |
| `tests/{cases,torch,pandas,pydantic,fastapi,stdlib,apps,numpy}/` | 486 / 162 / 138 / 59 / 43 / 134 / 57 / 8 .py files (apps: small / medium / large package programs); suites have `small/`, `models/`, a `PENDING` list. |
| `tests/run_snapshots.py`, `tests/snapshots/` | `--json` snapshots on four old modules: the internals regression guard. |
| `vscode-rowan/`, `rowan/py/` | VS Code extension (rowan serve); PyO3 binding (does not load on this machine). |

Directives (`tests/CONVENTIONS.md`): `#@ tier key`, `#@row NAME: present= definite= absent=
closed|open`, `#@value NAME: exactly=|unknown|includes=`, `#@schema src: always= sometimes=
only=`, floors `possible=` / `definite_within=`, `#@todo …`, `#@argv …`. A directive-only line
applies to the code line above. `possibly k` also holds when rowan's per-key verdicts differ
by run (the oracle reads it per line).

## How the engine works (the parts not in the spec)

* **Summaries + tabulation.** A function body is analysed once per key variant against
  placeholders and instantiated per call site; a call site is re-analysed concretely
  ("tabulated") when a recorded event is decidable there (dispatch, truth / `in` /
  `isinstance` / iteration / `.get` / arithmetic on a parameter, `concrete` events of models:
  a known text label — `csvhdr:`, `pyjsontext:` — decides one too). Module dicts / lists /
  sets that functions mutate are deferred loads in summaries (`deferred_global`), also when
  met through a callee's summary.
* **Object identity.** Site = module-prefixed line:col (+ class, `#i` in unrolled loops) and
  a context (call chain, receiver, `sum<f>v<n>`); hitting a live site again makes the object
  weak. Models that allocate several objects at one node in a Rust loop must give each its
  own context (pydantic: per input record / container; executor.map, object_hook: column
  offsets) or the second overwrites the first; a context a model passes to `alloc_at` is a
  suffix of the frame's (the same code run from two callers makes two objects). Single
  identity-hashed objects (futures,
  functions, instances without `__eq__`/`__hash__`) are exact dict keys (`identity_key`);
  objects in a key otherwise make it an unknown key (`opaque_key`).
* **Positions.** Literal containers, appends (also in branches / try bodies: lengths are per
  path, `State.lens`, joined to the shorter known prefix), decided comprehensions, sorts by
  literal keys, slices (also `slice()` objects; a slice of a placeholder is a list), yields,
  `zip`/`enumerate`/`map`; `lst[-1]` after an append of unknown position (`State.lasts`);
  `if items:` gives a list of unknown length a first element on that path (prefix 1, also
  through a filter-free comprehension; a frame of such records has their sure keys).
  Dicts keep insertion order; `True`/`1`/`1.0` and StrEnum members / their values are one key.
* **Values.** Exact on literals (`pyvalues`, `pynum`, `chrono`: Decimal / Fraction /
  datetime are literal keys too, compared by value in the `Compare` hook); comparisons,
  membership, `is None` narrows names; an unknown key tested `in` a closed dict of string keys
  narrows to its keys (`keynarrow` events through summaries); `flag = k in d` records the
  test's branch rows for a later `if flag:` (`flag_facts`); `while` / `match`; `CondFact`s;
  `Row.stored`, `State.name_keys`; entry runs and quiet requirement-only runs. Tuples,
  frozensets and frozen dataclass / pydantic instances with known items are value keys.
  Dict insertion order survives instantiation; an unknown order is never guessed (the
  iteration is then abstract). Rich comparisons call `__lt__` & co (`total_ordering`,
  `@dataclass(order=True)`, IntEnum / StrEnum by value). Narrowing never re-runs a call: each
  call node's last value is read back (`call_memo`); `obj.attr == lit` narrows the attribute.
  Keys removed from open records are absent (`Row::lacks`).
* **Stdlib objects** (`stdobj.<kind>` Namespace objects, attributes in Attr slots, methods via
  `m_stdobj`): StringIO, csv writers / readers, ExitStack, Template, Pattern / Match, path
  objects, sqlite connections / cursors. A parameter whose attribute is named like one of a
  family the program uses (`stdobj_attr_name`) decides per call site.
* **Classes.** Dataclasses (field specs, InitVar, KW_ONLY, frozen, replace), properties,
  C3 MRO, descriptors, `__missing__`, `vars()`, Enum (members as keys, auto, Flag ops,
  StrEnum), `cached_property`, `lru_cache`, singledispatch; namedtuple / itemgetter /
  attrgetter / re.match / UserDict / urllib.parse (urlparse, urljoin, parse_qs, urlencode,
  ported from CPython) are small generated modules.
* **Exceptions.** Raise states per try and class (`raise_cls`: names of the class's MRO,
  `EXC_BASES` for builtins; each handler starts from the raises it may catch that no earlier
  one surely catches, the rest go to the enclosing try; a model's raise inside a call is an
  exception object of its class, args unknown); builtin exceptions are objects with `args`,
  str / repr; a known out-of-range list index and a `del` of a missing key raise; a bare `raise`
  re-raises the handled value; a try body that may raise somewhere unknown gives its handler
  the requirements recorded up to there (`flag_reqs`); reads under a handler that always
  re-raises, or in threads / futures, stay requirements; `contextlib.suppress`; ExitStack;
  `@contextmanager` bodies run at the generator's `yield` (`CmHook`, also with a `then`
  callback: FastAPI yield dependencies); an exception from a callee of that body reaches the
  generator's handler with the generator's names. `finally`, `with` swallowing, unreachable
  after calls whose every path raises, `require_by_raise` as before.
* **Imports.** Local modules / packages, `--search-path`, `--follow` unless a model covers it.
* **Tensors** (SPECIFICATION §2.3, `tests/torch/CONVENTIONS.md`): `Data` labels `\x01T<shape>`
  etc.; conditions decided → `shape_errors`, undecided → `shape_reqs` + facts in `State.tf`;
  torch modules tabulate every function; hover renders `Tensor[B, T, 4*C]`.
* **pandas** (SPECIFICATION §11.2, `tests/pandas/CONVENTIONS.md`): Kind::Frame, columns in
  the Item row, meta slots (index, labels, values, dtype); tracked Series; pandas code in
  functions is tabulated (`concrete` events); readers accept a StringIO's text.
* **pydantic** (`tests/CONVENTIONS.md`): fields with aliases / defaults / Ann types,
  validators (field / model before & after / v1 root), serializers, computed fields,
  extra=allow/forbid, frozen, privates, generics, TypeAdapter, RootModel, settings;
  ValidationError objects (`errors()`); reads under a handler catching ValidationError are
  handled (no diagnostic). **FastAPI**: routes / routers / prefixes, handlers as entry
  points (through middlewares, run concretely), `fa_bind` (body / query / path / header /
  cookie, Enum and Literal validation → 422), dependencies (cached, route / router level,
  yield), TestClient (headers, cookie jar, query, status codes, 404 / 405, HTTPException
  detail, exception handlers), BackgroundTasks, request.state.
* **Soundness guards** worth knowing: a non-converged recursion widens its result and the
  rows it touched; `copy`/`deepcopy` of a parameter decides per call site; `**x` of an
  unknown value opens the row; deleting through an unknown key demotes every key; an
  unresolved placeholder's row is unknown; `isinstance` counts a value's literal part; a
  summary whose every path raises ends the caller's path (`completes`); a guarded frame column
  read records its KeyError before it narrows the frame to having the column.
* **Budgets.** 2 s soft deadline (no new tabulations), hard deadline 5× (`hard=`: user calls
  become unknown calls, entry runs stop); 256 tabulations per function / call, 256 clones per
  site, `MAX_ACTIVE` 4, 3 summary rounds, unroll 16 (torch 64), path 3. Profiling notes and
  the 10k+ line corpus: `SUGGESTED_CHANGES.md` (findings; in the JSON, `proven` = safe).

## Measured (release, Apple M-series)

| input | time | notes |
| --- | --- | --- |
| golden (1015 cases, all suites) | ~5 s | one process per case; oracle ~4 min |
| `j93` (830 lines) / `m96_pkg` (1,716) / `proto_summary.py` (1.2k, self-referential) | 0.09 / 0.10 / 3.6 s | pass / pass / 0 ✗ |

## Known problems and limits

* PyO3 wheel fails to load on macOS 26 / Xcode 26 (CLI / extension skip it); large
  self-referential programs are noisy; precision on real repositories never measured.
* A store through a name holding several objects is weak on each (`x346`/`x347` to-dos); an
  unknown call whose arguments hold records may alias them (its attribute row is open).
* Mutual recursion widens after 3 rounds; unknown `while` conditions join; `match` class /
  sequence patterns stay undecided; `getattr` needs a known name.
* Deletion by a key pattern (`for k in [k for k in os.environ if k.startswith(P)]: del …`)
  only demotes keys (sm02 / ym03 to-dos); record values do not flow from a list of records
  into a frame's columns / groupby keys (fm04 to-dos); a changed decimal context makes
  Decimal arithmetic unknown; `as_completed` order is unknown; `ExitStack.enter_context`
  runs a @contextmanager generator to its end when entered; a thread target runs at `start()`.
* Generators run eagerly per call site (tabulated); runs cut by a deadline depend on load.
* A slice by an unknown bound (`recs[:n]`, n from os.environ) has no positions (c001 to-dos:
  values known where positions exist would need a third `lens` field); pandas `sort_values` /
  `drop_duplicates` still count as "may raise anything" (mixed-type columns: TypeError).
* An exception from a summarised callee under a re-raising @contextmanager may reach a future
  without its value (`sm05` to-do); identity keys of an object turned weak stay in rows.
* Torch: dtype / device not modelled; numpy rides on the tensor model (numpy semantics where
  they differ: transpose, reductions, cumsum, `np.*`).
* pandas: no relations between columns; absent keys of open frames listed only when seen;
  values per row not simulated (b002 to-dos), frames keep per-column value sets.

## Where to go next

`SUGGESTED_CHANGES.md` (untracked): memory profile, abstract GC, on-demand server, violated hints
(owner's call); then c001 to-dos, possibly-unbound names, key-pattern deletion, torch dtypes.
