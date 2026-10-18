# Writing cases for the library suites

Suites `tests/pydantic`, `tests/fastapi`, `tests/stdlib`, `tests/apps` (and `tests/pandas`,
whose `CONVENTIONS.md` this extends). Layout: `small/` (one API area or one hypothesis each,
4–12 checks, 20–80 lines), `models/` (realistic programs, 120–300 lines, 20+ checks), and in
`tests/apps` `medium/` and `large/` package cases: a directory `a01_shop/` holding the entry
`a01_main.py` (its name shares the directory's prefix) and the modules it imports (any layout:
`shop/models.py`, `shop/api/routes.py`, …). Directives may sit in every file of a package case.

Expectations are written **by hand from the program text and the libraries' documented
behaviour, before rowan is run**; rowan is never used to produce or adjust them. Each case must
agree with the runtime oracle (strict mode):

```
uv run --no-project --with pandas --with pydantic --with pydantic-settings --with fastapi --with httpx --with numpy python tests/oracle.py tests/pydantic/small/y001_fields.py
```

New cases go into the suite's `PENDING` (one path per line, relative to the suite) until the
engine passes them. File names: `tests/pydantic/small/y001_…`, `models/ym01_…`;
`tests/fastapi/small/f001_…`, `models/fm01_…`; `tests/stdlib/small/s001_…`, `models/sm01_…`;
`tests/apps/{small,medium,large}/a001_…`.

## What rowan claims

A value is a set of possibilities: literals, objects, `?` unknown. Rowan
tracks **which keys** a record has — dict keys, instance attributes, DataFrame columns, model
fields — and the **values** of names when they follow from the program text.

* `#@row NAME: present={…} definite={…} [absent={…}] closed|open` — NAME's keys after the
  line ran: dict → keys; instance / pydantic model / dataclass / namespace → attributes
  (`vars(o)`); namedtuple → fields. `present`: every key that may be there; `definite`: there
  on every path; `absent`: surely not there (a key named here that the row has not seen counts
  when reading it there would surely fail); `closed`: nothing else possible, `open`: other
  keys may exist (data from input, unknown code, `**kwargs` of unknown origin).
* `#@value NAME: exactly={…}` (one of these literals) / `includes={…}` / `unknown` /
  `types={int,str}` (every unknown part is known to be of one of these types: `int(?)`; the
  oracle checks the runtime value's type) / `untyped` (some unknown part's type is not known).
* `#@general NAME: …` (the same clauses): NAME's value for **any** call of the function the line
  is in — the program's own, and any once the module has run, after any of its functions ran:
  its parameters anything their annotations allow, a method's receiver any instance (its
  `__init__` given any arguments; `__init__`'s own, of a class without subclasses: a new one). The oracle checks an `exactly` here against every call the
  program makes (they pass what the annotations say).
* `#@ TIER key` on an access line (`d["k"]`, `o.k`, `d.pop("k")`, `M(**d)`, …): `ok` (never
  fails), `missing` (always fails), `possibly` (some paths), `input` (depends on input data),
  `sound` (anything but missing), `fails` (anything but ok: the fuzzer's floor), `none` (no
  verdict: guarded / `.get`), `reported` (a finding: no handler guarding the read hides it). Several:
  `#@ ok a ; missing b`. A variable read is checked the same way (`#@ possibly x`: x bound on
  some paths only; the oracle counts a NameError / UnboundLocalError for x).
* `#@schema SOURCE: always={…} sometimes={…} only={…}` — what the program requires of an
  input: `json.loads@L<n>` / `json.load@L<n>` / `read_csv@L<n>` (the call on line n of this
  file), `param F.X` (parameter X of function F, for functions called with input from
  outside: handlers, callbacks), with a path for nested parts:
  `json.loads@L7['items']['*']`, `param handle.event['body']`. `always`: needed on every path;
  `sometimes`: on some; `only`: nothing else is required. The oracle drops each key from the
  input (at that call / at every call of F) and reruns: dropping an `always` key must raise a
  KeyError / AttributeError / pydantic missing-field error for it; dropping a key outside
  `only ∪ sometimes ∪ always` must not.
* `#@only param F.X: KEY if TEST` / `#@only param F.X: KEY none` — KEY of parameter X is read
  only on calls where TEST (Python over F's parameters, as passed) holds; `none`: no such test
  is claimed. The oracle drops KEY at every call of F: a call that raises for it must have had
  TEST true (evaluated at the call).

The precise answer is what holds for **every** run of the program, as written (the driver's
inputs are examples). Rules of thumb (the overview is in AGENTS.md):

* Literals are tracked through arithmetic, comparisons, string methods, formatting, `len` of
  known containers, comprehensions over known values, `sorted` / `min` / `max` of literals.
* Unknown inputs (`json.loads(text)` of a non-literal text, `os.environ`, files, request
  bodies, parameters of a function called from outside the program) are **open** records:
  nothing is known about their keys until an access succeeds (then the key is definite from
  there on: the program went on).
* A function's directives are checked for **every** call; rowan's answer is the join over its
  call sites (and an entry run with open parameters when nothing in the program calls it, or
  when a framework calls it: a FastAPI handler). Write what holds for all of them.
* Calls into code rowan cannot see (an unmodelled library) may change their arguments: such a
  record stays sound (`sound`, `possible=`, `definite_within=` floors are allowed where the
  precise answer is out of reach — but say the precise answer as a `#@todo` beside it).

## pydantic (v2; the v1 spellings `dict()`, `parse_obj`, `copy` too)

* `class M(BaseModel)`: the instance's attributes are the fields (annotated class attributes,
  inherited ones too; not `ClassVar`, not `_private`): closed; `model_config =
  ConfigDict(extra="allow")` → open (extra keys passed in become attributes); `extra="forbid"`
  → an unknown key is an error. Reading a non-field attribute: `missing`.
* `M(a=…, b=…)` / `M.model_validate(d)` / `M(**d)`: a required field (no default) missing →
  ValidationError: `#@ missing f` on that line for a known missing key, `possibly`/`input`
  when it depends on the data; `param F.d` / `json.loads@L` schemas require the required fields
  (by alias when the field has one; nested models: `['addr']`). Fields with a default are not
  required.
* `m.model_dump()` / `m.dict()`: a dict with the fields as keys (nested models → nested dicts),
  closed; `include={…}` / `exclude={…}` with literal sets select; `exclude_none=True` makes the
  fields that may be None present-only (`Optional` / `| None`); `exclude_unset=True` /
  `exclude_defaults=True` make the fields with defaults present-only; `by_alias=True` uses the
  aliases; `computed_field`s are keys too. `json.loads(m.model_dump_json())` has the same keys.
* `m.model_copy(update={…})`: same fields; `M.model_fields`: dict keyed by field names;
  `M.model_json_schema()["properties"]`: keyed by field names (aliases).
* Validators run user code: `@field_validator("x")` gets the value, `@model_validator(mode=
  "before")` gets the raw input (a dict: its accesses are requirements of the input),
  `mode="after"` gets the instance.
* `TypeAdapter(list[M]).validate_python(data)` → a list of M; `RootModel`; `pydantic.dataclasses`;
  `pydantic_settings.BaseSettings` (fields from the environment; its instance like a model).

## FastAPI

* Handlers (`@app.get/post/put/patch/delete(...)`, `@router.…`, `APIRouter` +
  `include_router`) are called by the framework: rowan analyses them as entry points with
  parameters bound the FastAPI way (and from `TestClient` calls in the driver):
  a parameter annotated with a pydantic model → a validated instance (its fields);
  `dict` / `Dict[str, Any]` / `Body(...)` → an open record (`param F.x` schema source);
  `request: Request` → `await request.json()` open; scalars / `Query` / `Path` / `Header` →
  unknown values (plus the default); `Depends(g)` → g's return value (g's own parameters bound
  the same way; a `yield` dependency: the yielded value).
* `TestClient(app).post("/items", json={…})`: runs the matching handler; `.json()` is the
  returned value (a model dumped to a dict).
* `raise HTTPException(...)` raises; a KeyError inside a handler reaches the TestClient call.

## Standard library and builtins

Precise where the text decides it: `dict` methods / `|` / comprehensions, `collections`
(`Counter`, `defaultdict`, `OrderedDict`, `ChainMap`, `namedtuple`, `deque`), `dataclasses`
(`asdict`, `astuple`, `replace`, `fields`, `make_dataclass`), `enum`, `typing.TypedDict` /
`NamedTuple`, `types.SimpleNamespace` / `MappingProxyType`, `json` round trips of literals,
`csv.DictReader` (open rows; `fieldnames=` closes them), `configparser`, `argparse`,
`os.environ` (open), `string.Template` / `str.format_map` / `"%(k)s" % d` (read keys),
`operator.itemgetter` / `attrgetter`, `functools` (`partial`, `reduce`, `wraps`, `lru_cache`,
`cached_property`, `singledispatch`), `itertools`, `contextlib` (`suppress`, `contextmanager`,
`ExitStack`), `copy`, `re` on literal patterns, `math` / `statistics` / `operator` on literals,
`sorted` / `min` / `max` with `key=`, `map` / `filter` / `zip` / `enumerate`. Callbacks given to
library functions (`key=`, `map`, `executor.submit(f, …)`, `Thread(target=f)`,
`re.sub(pat, f, s)`, `defaultdict(factory)`) are analysed with the arguments the library passes.

## Program shape

* Functions with directives plus a driver under `if __name__ == "__main__":` that calls
  **every** function **with several different inputs** (2–4: different key sets, empty
  inputs, extra keys, values that take different branches). Every claim inside a function must
  hold for all of them. Module-level code counts too.
* Every directive line must run (strict oracle); the program must not crash (catch deliberate
  failures in the driver: `try: … except (KeyError, AttributeError, ValueError): pass`;
  pydantic's ValidationError is a ValueError).
* No network, no files outside the case directory (data files next to the case are fine;
  write temporary files under `tempfile`), deterministic (no time / random dependence in
  claims), each case runs in under a few seconds.
* Put the directive at the end of the statement's line, or on a comment line of its own
  directly below. A directive line applies to the code line directly above it, even when that
  line is inside an `if` / `for` body: to check after the block, write a statement after it
  (`_ = 0  #@row out: …`). Keys: bare words or Python literals (`"Amount Due"`, `0`).
* One-line header comment saying what the case probes.
