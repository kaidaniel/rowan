# PLAN — pydantic, FastAPI, the standard library, mixed programs, numpy

Owner's request (2026-10-09): support pydantic and FastAPI; go through the standard library and
builtins and cover (most of) their APIs in models; then several rounds of new cases probing what
may not work yet — programs of all sizes, with imports and external dependencies, mixing models
(pydantic + FastAPI, pydantic + pandas), focused on analysis *inside functions* and calls across
modules where the inputs are only known at run time; squash every bug found. Then a numpy model
(reusing the tensor shapes).

Conventions as before: expectations are written by hand from the program text (by agents that
never run rowan) and checked against a real run (the oracle) before the engine is touched; one
commit per step at a passing state; new cases sit in their suite's `PENDING` until they pass.

## 0. Harness

* New suites (strict oracle, `small/` + `models/` like pandas): `tests/pydantic`,
  `tests/fastapi`, `tests/stdlib`, `tests/apps` (multi-module programs: a case is a directory
  whose entry shares its prefix, as in `tests/cases`), `tests/numpy`. `run_golden.py` and
  `oracle.py` discover them; the oracle runs with
  `--with pandas --with pydantic --with fastapi --with httpx --with numpy`.
* Oracle extensions: pydantic `ValidationError` (a `missing` error at `loc=(k,)`) counts as a
  raise for `k`; parameter schemas (`#@schema param f.x`): rerun with key k removed from x's
  dict at each call of f (PEP 667 write-through `f_locals` at the call event); `json.loads@L` /
  `json.load@L` / `os.environ` schemas by the same mutation of that call's result.

## 1. pydantic (v2, with the v1 spellings)

Classes deriving `BaseModel` (and `pydantic_settings.BaseSettings`, `RootModel`,
`pydantic.dataclasses.dataclass`): fields from annotations (`Field(default=…, default_factory=…,
alias=…)`, `Optional`, `ClassVar` / private `_x` excluded), inheritance, nested models.
* Construction: required fields missing from a closed call → error verdict (ValidationError);
  `model_validate(d)` / `parse_obj` / `model_validate_json(s)` / `TypeAdapter(T).validate_python`
  require the required fields of `d` (schema of the input); `extra="forbid"|"allow"|"ignore"`;
  `model_construct` (no validation).
* Instances: declared attribute row (closed unless `extra="allow"`); `model_dump` / `dict`
  (`include`, `exclude`, `exclude_none/unset/defaults` → present-only, `by_alias` → alias keys),
  `model_dump_json` + `json.loads`, `model_copy(update=…)`, `model_fields` /
  `model_fields_set`, `model_json_schema()['properties']`, validators (`field_validator`,
  `model_validator(mode="before")` sees the raw dict, `computed_field` adds a dump key).

## 2. FastAPI

`FastAPI()` / `APIRouter()` route decorators (`get/post/put/patch/delete/api_route`) mark the
handler as an entry point whose parameters are bound the FastAPI way: a pydantic-annotated
parameter → a validated instance of the model; `dict` / `Body()` / `await request.json()` → an
open JSON record (schema source `param f.x`); `Depends(g)` → g's result (g analysed with its
own parameters bound the same way, nested); `Query/Path/Header/Cookie/Form` → unknown scalars
(defaults join in). `response_model=` / the return annotation: the returned row is checked
against the declared model (missing required field → error). `HTTPException` raises;
`include_router`, middleware and event decorators register; `TestClient(app).get/post(…)`
dispatches to the matching route with the JSON / query / path values, so drivers make concrete
calls (`.json()` = the handler's return dumped).

## 3. Standard library and builtins

Audit every builtin and the commonly used modules; write the coverage table into
SPECIFICATION §11 (modelled precisely / value-only / no-effect / unknown). Priorities, in order:
1. calls that run user code: `sorted/min/max(key=)`, `map/filter`, `functools.reduce /
   partial / wraps / lru_cache / cache / singledispatch / cached_property`, `itertools.*`
   (`groupby key`, `starmap`, `accumulate`, `product`, `islice`, `zip_longest`, `takewhile`…),
   `concurrent.futures` `submit / map`, `threading.Thread(target=)`, `atexit.register`,
   `contextlib.contextmanager / suppress / ExitStack / closing / nullcontext`, `re.sub` with a
   function, `heapq` with keys, `defaultdict(factory)`, `dict.setdefault`.
2. calls that read keys (requirements): `str.format_map`, `"%(k)s" % d`, `string.Template
   .substitute` (vs `safe_substitute`), `operator.itemgetter`, `str.format(**d)`,
   `configparser` sections / options, `csv.DictWriter(fieldnames)` with extra keys (ValueError).
3. calls that build or transform records: `dict` methods and `|`, `collections`
   (`Counter` arithmetic / most_common, `ChainMap` lookups, `deque`, `OrderedDict`,
   `UserDict`), `types.MappingProxyType`, `json` round trips, `csv`, `sqlite3.Row`,
   `dataclasses` (`astuple`, `make_dataclass`, `fields`), `enum` members, `typing.TypedDict`,
   `os.environ` (open mapping), `argparse`, `shelve`, `copy`.
4. values: literal arithmetic in `math` / `operator` / `statistics`, `str` / `bytes` methods,
   `set` algebra on literal sets, `divmod/pow/chr/ord/hex/bin/oct`, `os.path` / `pathlib` on
   literal strings, `string` constants, `re.findall / split / sub / groupdict` with minire.
4. soundness of the no-effect list: nothing on it may mutate an argument record.
Small cases for each area (`tests/stdlib/small`), written before the models.

## 4. Hypothesis rounds (repeat until rounds stop finding bugs)

Each round: ~20 new cases across sizes — small (one hypothesis), medium (a module or two,
100–300 lines), large (a package of 5–10 modules, 500–1,500 lines, external dependencies:
pydantic + FastAPI services, pydantic + pandas ETL, CLI tools over json / csv / argparse,
plugin registries, config loaders), all oracle-checked, then every failure fixed in the engine
(or the expectation corrected with a comment). Focus: functions analysed with inputs that only
the runtime decides (handlers, callbacks, parsed config, data files), functions in other
modules calling each other, summaries vs tabulation, classes across modules.

## 5. numpy

`np.array / zeros / ones / arange / linspace / random.*`, reshape / transpose / broadcasting /
matmul / reductions with `axis` / `keepdims`, concatenate / stack, indexing and slicing, boolean
masks, `np.where`, structured arrays (`dtype=[("a", …)]`: field names are keys, `arr["a"]`
verdicts, `genfromtxt(names=True)` open), on the torch shape machinery (`shape.rs`); a numpy
oracle in `tests/torch_oracle.py`'s style (`#@shape` on ndarrays). Small cases + a few models.

## 6. Docs

README, SPECIFICATION (§11 tables), AGENTS.md (under 200 lines); remove this plan when done.
