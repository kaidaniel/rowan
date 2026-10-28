# rowan/src/models

The library models: what calls of the standard library, pandas, pydantic, FastAPI, PyTorch and
numpy do to the values they are given. Engine logic does not belong here; a model only says
what one library call does.

## What a model is

A model is a function `fn(&mut Analyzer, &mut State, &Call) -> Value` (`mod.rs` `Model`). The
interpreter (`interp/calls.rs` `call_external`, `call_method_object`) finds it by the callee's
qualified name (`json.loads`, `dict.get`, `DataFrame.merge`) in the `Registry`
(`mod.rs`): an exact name, a pandas method's, then a family's prefix (`str.*`, `tensor.*`);
the libraries known to change nothing they are given get `noeffect`; anything else is an
unknown call (`interp/unknown.rs`).

A model computes the result on known values (CPython's, exactly), writes what the call does to
the state (stores, a list's new length, an open row for data it cannot know), and may raise
(`an.raise_exc`). It uses the `Heap`, its own side tables (`Tables` in `mod.rs`, which the
collector follows) and the interpreter's `pub` methods listed in `interp/api.rs`. Objects a
model makes at one node each need a context of their own (`AGENTS.md`, rules easy to break).

`tables.rs` is what the interpreter knows of library functions without running a model: which
raise only on arguments of a wrong type, which have no effect, which take an iterable's items
at once.

## Files

| file | models |
| --- | --- |
| `mod.rs` | the registry, module constants (`sys.maxsize`, `string.digits`), `os.environ`, `noeffect`, builtins called through dunders, `ClassBase` (what library bases make of a class), the side tables, attributes of model objects |
| **builtins and the standard library** | |
| `builtins.rs` | builtin functions (`dict()`, `isinstance`, `getattr`, `min` / `max`, `zip`, `len`, `sorted`'s order, …) and `str` methods |
| `containers.rs` | `dict.*`, `list.*`, `set.*` methods, ChainMap (read through its maps as they are now: `Analyzer::sync_chainmaps`) |
| `stdlib.rs` | copies, namedtuple, SimpleNamespace, `asyncio.run` / `gather` / `wait_for` / `create_task` (awaiting the coroutines they are given), `operator` getters, `globals` / `locals`, `exec`, dataclass helpers, `defaultdict`, `DictReader`, `functools.partial`, `itertools.chain`, logging, `os.getenv`, `get_type_hints`, number and datetime constructors, `collections.UserDict` and `urllib.parse` analysed from Python source |
| `json.rs` | `json.dumps` / `loads`, HTTP bodies and responses |
| `pure.rs` | calls whose arguments are all literals, computed by `pyvalues.rs` |
| `pyvalues.rs` | Python operations on literal values exactly as CPython 3.14 does (formatting, arithmetic, str methods, …) |
| `pytext.rs` | text and bytes functions on literals (`bytes`, `shlex`, `html`, `base64`, quoting) |
| `pathtext.rs` | `os.path` and pure `pathlib` paths on known text (a path of unknown text: its parts typed) |
| `pynum.rs`, `chrono.rs` | `decimal` / `fractions` and `datetime` values as literals |
| `pyre.rs`, `remodel.rs` | a regex engine with Python's syntax; `re` on known patterns |
| `minire.rs` | the small regex matcher pandas' regex arguments use |
| `csvio.rs` | `io.StringIO` and `csv` on known text; file objects (`open()`: reads of str / bytes) |
| `sqlite.rs` | `sqlite3` connections simulating simple SQL |
| `configparser.rs`, `argparse.rs` | INI parsing; argument parsers on a known argv |
| `counter.rs` | `collections.Counter` |
| `dataclasses.rs` | generated `__init__` of dataclasses, NamedTuple, TypedDict; abstract classes |
| `enums.rs`, `exceptions.rs` | Enum members and Flags; builtin exceptions |
| `itertools.rs`, `stdfuncs.rs`, `singledispatch.rs` | itertools; heapq, bisect, `operator` callables; `singledispatch` and `lru_cache` |
| `concurrency.rs` | threads and executors running their targets |
| **third-party libraries** | |
| `pandas.rs` | DataFrames and Series: columns through every operation, readers open through their input; the literals a column holds (in row order for literal data) through iteration, cells and stores |
| `pydantic.rs` | models, fields, validation, validators, dumps, settings |
| `fastapi.rs` | apps, routes, parameter binding, dependencies, the test client |
| `torch.rs`, `shape.rs` | PyTorch and numpy: tensors with symbolic shapes (`shape.rs`: dims as polynomials, constraints, the facts a path carries) |

## Adding a model

Write the function where its library's models are, register it under its qualified name in
that file's `install` (pandas, pydantic, FastAPI) or in `Registry::new`, and add a golden
case under `tests/` whose expectations are written from the program text first (`AGENTS.md`,
Tests). A model that is wrong is unsound: the runtime oracle (`tests/oracle.py`) runs every
case under CPython and checks what rowan claims.
