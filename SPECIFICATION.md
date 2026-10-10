# rowan: specification

How rowan works, as implemented in `rowan/` (Rust). The README shows what it does; this
document says how and with what guarantees. Code names in `backticks` point into
`rowan/src/`.

## 1. Overview

rowan is a whole-program **abstract interpreter** for Python. It parses the program (ruff's
parser) and walks the syntax tree directly, without an intermediate representation, executing
it on abstract values:

* a **value** is a set of possibilities: literals, abstract objects, or *unknown* (§2);
* **objects** live in an abstract heap, identified by allocation site and context (§3);
* the **state** at a program point maps names to values and objects to what is known about
  their contents (§4);
* both sides of every branch run, and their states are joined. Loops run to a fixpoint or
  are unrolled (§5). Calls are interpreted per call site where that matters, and through
  reusable summaries elsewhere (§6). Exceptions (§7) and generators (§8) follow their real
  control flow;
* library calls go to **models** that compute what the library would, exactly on known
  values (§11);
* along the way rowan records **requirements**, the keys the program reads from values that
  came from outside (§9), and **verdicts** on every access (§10).

Design principles:

* **One value domain.** Dicts, instances, lists, frames, tensors, strings and numbers are all
  values. What is known about an object's contents comes from the model for its kind. An
  unknown is the same `?` everywhere.
* **Facts live on objects, not on names.** Aliasing is a question of which objects a name may
  hold.
* **Sound by default, attributed when not.** Where a result depends on code rowan cannot see,
  the uncertainty is recorded with its cause (a `mutator`: "unknown call `lib.f` at L10"), and
  the sound and the optimistic reading are kept side by side (§10).
* **Exact on known data.** A library call on known values computes CPython's result: string
  methods, formatting, arithmetic, `json.dumps` text, regex matches, SQL queries,
  `datetime` arithmetic, pandas columns, tensor shapes.

## 2. Values

`Value { pts, kv }` (`domain.rs`): `pts` is a set of object ids (§3), `kv` the scalar part:

| `KeyVal` | meaning |
| --- | --- |
| `Bot` | no scalar possibility (the value is only objects) |
| `Lit(k)` | one literal |
| `Lits(S)` | one of a finite set of literals (at most `MAX_LITS` = 64, beyond that `Data`) |
| `KeysOf(objs, ns)` | one of the keys of these objects (a loop over `d`, `d.keys()`, `df.columns`) |
| `Data(label)` | unknown. The label names it in the program's terms (`row["id"]`, `x.shape[0]`, `json.loads(text)`), types it when known (`str`, `int`, `len`), or encodes a tensor shape (§11.4) |
| `Param(ph)` | the unknown value of a summary placeholder (§6.1) |

**Literals** (`intern.rs`, interned to `KeyId`) are `str`, `bytes`, `int`, `float`, `bool`,
`None`, tuples and frozensets of literals, `Decimal` and `Fraction` (compared by value),
`datetime` values, Enum members, and the identity of single objects that hash by identity
(functions, instances without `__eq__` / `__hash__`). `True`, `1` and `1.0` are one dict key, as
in Python.

### 2.1 Notation

Values print as Python-like text (text report, editor hover, `--facts`):

| shown | meaning |
| --- | --- |
| `1`, `"a"`, `None` | a literal |
| `1 \| 2 \| ?` | one of several possibilities |
| `?` | nothing known |
| `a`, `cfg["db"]`, `rows[*].name`, `json.loads(text)`, `1 + b` | an unknown named by where it comes from. Printed only where the names mean the same (inside their function), elsewhere `?`; the hover's legend says what is known about each |
| `{"id": 1, "role"?: "admin", "x"~: ?}` | a dict with these keys: `id` always, `role` on some paths, `x` unless unknown code removed it |
| `{"id": 1, **?}` | the same, open: other keys may exist |
| `Order(id=1, **?)` | an instance and its attributes, same marks |
| `[1, ?]`, `(1,)`, `list[X]`, `set[X]` | known positions; any number of elements like `X` |
| `Tensor[B, T, 4*C]`, `Tensor[*x.shape[:-1], 10]` | a tensor shape (§11.4) |
| `{…}`, `[…]`, `…` | cut by the printer (depth, more than eight entries, a cycle) |

## 3. Objects

An object (`Obj`, `interp/mod.rs`) has a **kind** (`Kind`: Dict, List, Tuple, Set, Instance,
Class, Function, Module, Frame, Series, Ph(aceholder), Unknown, …), a **payload** with
kind-specific data (a function's code and closure cells, an instance's class, a view's dict),
and an identity:

* **Site and context.** An allocation is identified by its site (the module-prefixed
  `line:col`, plus `#i` per unrolled loop iteration) and a context: the call chain (up to
  `chain` = 2 levels, hashed beyond), the receiver for methods, `sum<f>v<n>` inside a summary.
  Executing the same site in the same context returns the same object.
* **Single objects and weak ones.** An object that stands for at most one runtime object is
  *single*. Stores into it are strong: they replace the old value. Allocating a site again
  while its earlier object is still live (reachable from the state, `live_among`) gives a
  *variant* (`ctx~1` … `ctx~8`). Past eight, the site's object is *merged*: rows and slots
  then only join, which keeps a store into one runtime object from overwriting the others.
* **Recency.** Inside a loop, the object allocated by the current iteration is a strong
  recent object, folded into the site's summary object at the back edge.
* Objects are never removed from the table: verdicts and facts refer to them. Garbage
  collection (§6.7) removes their *state*, not the objects.

## 4. State

`State` (`state.rs`) is one program point on one path:

| field | holds |
| --- | --- |
| `env` | local names → values |
| `rows` | `(object, namespace)` → `Row`: the key-set model (§4.1) |
| `slots` | object → `(namespace, key)` → value: what is stored under each key |
| `elems` | object → the scalar part of its elements |
| `lens` | object → (known length, known prefix), per path |
| `lasts` | object → the element appended last (`xs[-1]` after appends of unknown position) |
| `reqs` | source → key → requirement (§9) |
| `facts` | conditional facts: rows a branch had, restored when the same test recurs (§5.1) |
| `tf` | tensor facts: ranks and dim equalities on this path (§11.4) |
| `unbound` | locals bound on some paths only: a name one side of a join bound and the other did not |
| `ret`, `exits` | the returned value so far; the states of paths that returned |
| `reachable` | false after a `raise`, `return`, `break`, or a call whose every path raises |

**Namespaces** (`Ns`): `Item` (`d[k]`, frame columns), `Attr` (`o.k`), `Cell` (closure cells),
`Pos` (list and tuple positions), `Elem` (any element), `Meta` (pandas index, labels, dtypes).

**Persistence** (`layered.rs`). The maps are persistent big-endian Patricia tries on integer
keys (Okasaki and Gill, as in SPARTA's `PatriciaTreeMap`):
* copying a state is O(1);
* a write copies the path to its leaf (in place when no copy shares it);
* `join`, `same` and `diff_keys` skip the subtrees two states share, so they cost what the
  states differ in.

`slots` and `reqs` keep a copy-on-write hash map per object or source (`TwoLevel`).

**Joins.** At a merge point two states join pointwise: names join their values (a name bound on
one side only keeps that side's value), rows and slots join, lengths join to the shorter known
prefix, facts both sides hold are kept.

### 4.1 Rows: the key-set model

A `Row` describes the keys of one object in one namespace (`RowData`, `domain.rs`):

| field | meaning |
| --- | --- |
| `present` | keys that may be there |
| `definite` | keys there on every path (`⊆ present`) |
| `likely` | keys there unless unknown code removed them (`⊇ definite`) |
| `absent`, `absent_likely` | keys surely (likely) not there: deleted, renamed away, projected out |
| `sources` | open sources the row may hold keys of (an input, a data key), each with a *lens* (renames, kills, prefixes applied since) |
| `mutators` | unknown code that may have changed the object (e.g. an unknown call at L10) |
| `top` | nothing known |
| `terms`, `dterms`, `lterms`, `pstores` | in summaries: keys of a placeholder's entry row (present / definite / likely), and stores whose key is a placeholder's value (§6.1) |
| `order` | insertion order of the keys stored strongly (dict iteration order) |

A row with no sources, mutators or `top` is **closed**: its keys are exactly `present`. Joins
take the union of `present`, `sources` and `mutators` and the intersection of `definite`,
`likely` and `absent`.

Transfers:
* a strong store adds the key to `present`, `definite` and `likely`; a weak store adds it to
  `present` only;
* a strong delete adds the key to `absent`;
* copying a record (`{**r}`, `dict(r)`, `update`) adds its keys, terms and sources;
* an unknown call adds a mutator and empties `definite`, keeping `likely` (§6.5);
* a store under an unknown key (`d[f(x)] = 1`) adds a data source;
* a successful read narrows the key in: after `r["x"]` the program went on, so `x` was there.

### 4.2 Positions and elements

Lists and tuples keep positions in `Pos` slots while they are known:
* literal containers, appends (also in branches: the length is per path), decided
  comprehensions, sorts by literal keys, slices, `zip` / `enumerate` / `map` over known items;
* `if items:` gives a list of unknown length a first element on that path;
* a list of unknown length keeps the *prefix* that is known on every path;
* otherwise one `Elem` slot holds the join of all elements.

Dicts keep insertion order (`Row.order`). Iterating a dict whose keys are known in order runs
each key, a key present only on some paths both run and skipped.

## 5. Statements

### 5.1 Branches and narrowing

Both branches of an `if` run on copies of the state unless the test is decided. The test
narrows each branch:

* `k in d` / `k not in d`, `d.get(k) is None`, `hasattr` narrow the row;
* `x is None` and comparisons with literals narrow the name;
* `isinstance(x, C)` narrows to objects of `C` (an unknown class keeps everything);
* truthiness drops `None`, `0`, `""` and known-empty containers on the true branch;
* `obj.attr == lit` narrows the attribute.

An unknown key tested `in` a closed dict of string keys narrows to those keys. A test assigned
to a name (`flag = k in d`) narrows when the name is tested later. When the same test recurs
after a join (`if flag: … if flag: …`), each branch gets back the rows it had (`CondFact`).
`while` and `match` narrow the same way; a `match` case that cannot match is skipped, one that
surely matches ends the match.

### 5.2 Loops

* A loop over a known iterable of at most `UNROLL_MAX` (16) items is **unrolled**: each
  iteration runs with its item, and `break` / `continue` rejoin where Python would.
* The known prefix of a list of unknown length runs first, unrolled; the rest runs as a
  fixpoint.
* Other loops iterate to a **fixpoint**: the head joins the entry state with the back edge
  until it stops changing, at most `LOOP_MAX_ITER` (40) rounds. At the cap, what still changes
  is widened (open rows, unknown values), so the exit state is a post-fixpoint.
* A loop over a dict's keys that stores under every key leaves those keys definite.
* After a loop, a later `if xs:` gets the rows the iterations left (the loop ran at least once
  exactly when the iterable was non-empty).

### 5.3 Other statements

* `with` calls `__enter__` / `__exit__`. A `@contextmanager` runs its generator, and the `with`
  body runs at every `yield` the generator reaches, each from that yield's state; the caller's
  names after the `with` join over them. An `__exit__` returning true swallows the exception.
* Comprehensions have their own scope, decided when their iterable is known.
* `del`, augmented assignment, star unpacking, the walrus operator, `global` / `nonlocal`
  (through module slots and cells) and class bodies run as written.

## 6. Calls

### 6.1 Summaries

A function body is analysed once per *signature variant* against **placeholders**: one per
parameter, `self`, closure cell, and each access path below them that the body touches
(`self.store`, `cfg["db"]`), up to `PATH_DEPTH` (3). The variant is the classes of the receiver
and of instance arguments, and the code in the closure cells, up to `MAX_SIGNATURES` (8). The
result, a `Summary` (`summary.rs`), holds:

* the **footprint**: the rows, slots and elements that differ between entry and exit, for
  objects the caller can reach;
* the returned value; the requirements recorded on placeholders; verdicts relative to the
  placeholders;
* **events**: places where the body lost information about a placeholder (§6.2);
* whether it may raise, the classes it may raise (§7), and whether any path completes.

At a call site the summary is **instantiated**:
1. placeholders are bound to the caller's objects and values;
2. the footprint is substituted and applied: strong when a placeholder is bound to one single
   object, joined otherwise;
3. objects allocated in the body are cloned into the call's context;
4. requirements and verdicts are re-expressed in the caller's terms.

A summary whose every path raises ends the caller's path.

### 6.2 Tabulation: re-analysing a call site

Instantiation is exact only where the body did not depend on what the placeholders hold. The
body records an **event** wherever it did:
* a call or method dispatch through a parameter;
* `isinstance`, truthiness, `in`, iteration, `.get`, arithmetic or formatting on a parameter;
* a subscript of a parameter by position;
* a model's `concrete` event (a pandas call on a parameter, a known text a reader parses);
* an attribute read through a parameter whose class comes only from its annotation (`hint`).

When a call site's arguments decide one of these events (a real function, a literal, a known
container), the body is run concretely at that call site instead: it is **tabulated**.
Tabulation is limited to 256 runs per function and event kind, 8 runs of a recursive call at
one site inside a loop, and the soft deadline (§6.8). The **call cache** replays a tabulated
call's footprint when the same callee meets the same observable input again.

### 6.3 Recursion

A function reached again while it is being analysed uses the partial summary of the previous
round. The function iterates until its return value, exit state and requirements are stable,
for at most `MAX_SUMMARY_ROUNDS` (3) rounds in summaries and `REC_MAX_ITER` (8) when tabulated.
A recursion that does not converge widens its result and the rows it touched. At most
`MAX_ACTIVE` (4) activations of one function are run at once.

### 6.4 Entry points

Functions nobody in the program calls (library code, handlers, CLI mains) are analysed with
open parameters: their requirements become schemas (`param F.x`), and their variables get
results too. Functions that are called also get a quiet, requirement-only run with open
parameters, which gives their own contract. FastAPI handlers are entry points with an open
request (§11.3). Torch modules analyse every function on its own.

### 6.5 Unknown calls

A call rowan cannot resolve or model (an unknown import, a dynamic callee):
* records a **mutator** on every record reachable from its arguments and receiver (at most
  `MAX_MUTATED`, 24), which drops their `definite` keys but keeps `likely`;
* returns a fresh unknown object whose rows join the argument records (an unknown call may
  return one of them). With more than `MAX_ARG_JOIN` (64) records it returns an open row
  instead;
* may raise anything;
* is listed in the report's notes.

A small set of calls known not to touch their arguments (`print`, logging, `len`, `str`, …) has
no effect.

### 6.6 Classes, closures, decorators

* **Classes:** C3 MRO, properties and descriptors, `__getattr__`, `__getitem__` / `__iter__` /
  `__missing__` / `__eq__` / `__lt__` and the other protocol methods, `super()`, class
  attributes.
* **Dataclasses** (field specs, `InitVar`, `KW_ONLY`, `frozen`, `order`, `replace`), Enum
  (members are keys), NamedTuple and TypedDict.
* **Closures** capture cells: two closures from one factory call have their own cells.
* **Decorators** run as calls; `functools.wraps`, `lru_cache`, `cached_property`,
  `singledispatch`, `total_ordering`, `property`, `staticmethod` and `classmethod` are
  modelled.
* **Annotations.** An annotation types a placeholder only through wrappers that keep the
  value's class (`Optional`, `Annotated`, `Final`, `ClassVar`, a union with `None` and one
  class). `list[Foo]` is a list, not a `Foo`; a union of several classes types nothing. A
  hint is trusted only where the class is unknown: the arguments' classes at a call take
  precedence (the summary is specialised on them), and a call site that passes an instance of
  another class than the hint's runs the body concretely (a `hint` event, §6.2).

### 6.7 Garbage collection

At every `return`, in tabulated calls and summary runs, objects the call allocated that are no
longer reachable lose their state (`drop_garbage_at_return`). Dead objects then stop costing
joins and comparisons, and allocation sites stay strong (§3). Reachability starts from these
roots:
* the returned value;
* what older objects' changed slots hold (globals, parameters' records, cells);
* the call's placeholders and unknown objects, since they stand for objects outside the call;
* the analyzer's side tables and value stacks (`gc_pinned`).

It follows slots, element kinds, payloads and side tables (`obj_edges`). Parts of an input
(unknown objects, path placeholders) are never collected: the same path finds them again by
their site. `ROWAN_GC=0` restricts collection to returns of values without objects.

### 6.8 Deadlines

* **Soft deadline**, `ms` = 2 s: no new tabulations; summaries are used instead.
* **Hard deadline**, `hard` = 5 × the soft one: calls of user code become unknown calls, and
  entry runs stop. Calls already running stop too: their bodies end where they are, and each
  such call is replaced in its caller by an unknown call (on the state it had reached), so the
  run ends in bounded time with a sound but less precise result.

With `ROWAN_LIMITS=work=N` both deadlines count work instead of time (statements run, summary
templates instantiated, objects visited by reachability walks at 16 to a unit; soft at N, hard
at 5N): a run cut by them gives the same result on any machine and under any load. About
270,000 units take a second on an Apple M-series machine.

Both are set through `ROWAN_LIMITS`. With the default clock deadlines, the results of a run
cut by a deadline depend on the machine's speed and load.

## 7. Exceptions

Each `try` collects **raise states**: the state at each point that may raise, per exception
class.
* A class is named by its MRO; builtin classes come with their bases (`EXC_BASES`), and
  library classes are module-qualified.
* Each handler starts from the raises it may catch that no earlier handler surely catches; the
  rest go to the enclosing `try`, or leave the call (a summary records which classes).
* An exception object carries its `args`, `str` and `repr`, and model raises carry their real
  attributes (a `JSONDecodeError`'s `msg`, `pos`, `lineno`, `colno`; a pydantic
  `ValidationError`'s `errors()`).
* A bare `raise` re-raises the handled value.
* `finally` runs on every way out. `contextlib.suppress`, `ExitStack` and `with` blocks that
  swallow work as at runtime.
* A read inside a `try` whose handler catches its `KeyError` still records the key as
  required: the exception fires when the key is missing, whether caught or not (§9).
* A frame column read that may fail records its `KeyError` before it narrows the frame to
  having the column, so the handler starts from before the read.

## 8. Generators and iterators

Calling a generator function runs its body eagerly, at the call site. The items are a list
object (`gen:<f>`), with known positions where the yields are straight-line or in decided
branches, and a known prefix for endless generators.

* **One-shot.** A generator keeps a cursor: a loop or `next` consumes items, and a `break`
  leaves the rest for a later consumer.
* **Effects per item.** Objects the body changes before each yield hold, for a consumer, the
  state at that yield (`GenEffects`). A consumer that takes item *k* sees the effects up to
  yield *k*.
* **Lazy wrappers.** `zip`, `map`, `filter` and `islice` over generators pull items as CPython
  would (`zip` stops at the shortest input).
* `yield from` delegates and evaluates to the generator's return value.
* A bare `yield` yields `None`. A `yield` expression's value is `None` (iteration sends nothing),
  or unknown when the program calls `.send(` anywhere.
* **Iterator protocol.** An iterator class whose `__iter__` returns `self` is iterated through
  `__next__`.

## 9. Requirements and schemas

A read of a key from an object whose row includes an open source (an input) is a
**requirement** on that source.

* **Sources.** `json.load(s)`, `read_csv` and the other readers, `os.environ`, request bodies,
  the parameters of entry points, unknown call results; the open parts of their contents are
  sources too (`json.loads@L8['database']`, `param f.rows['*']`). Lenses map a key read after a
  rename back to the source's own name.
* **always / sometimes.** A key read on every path from the source's creation is `always`, one
  read on some paths only `sometimes`. Reads in a loop over a source's elements are `always`
  per element. A read behind `if k in src`, or with `.get`, requires nothing.
* **Guarded reads.** A read under a handler that catches its `KeyError` is still required:
  dropping the key makes the exception fire. That is the rule the runtime oracle checks.
* **Propagation.** Requirements recorded on a summary's placeholders are substituted at each
  call into the caller's sources. A function's own contract is its `param F.x` schema.
* **Disjunctions.** A key provided by one of several sources, or under one of several names,
  is reported as alternatives (`'amount' | 'amt'`).

## 10. Verdicts

Every key or attribute read, `del` and `pop` gets a verdict per target object, from that
object's row (`classify`, `domain.rs`):

| tier | when | reading |
| --- | --- | --- |
| `proven` ✓ | the key is `definite` | sound |
| `likely` ✓~ | `likely` but not `definite`: an unknown call ran since it was set | optimistic |
| `possibly` ⚠ | `present` but not `definite` | sound |
| `missing` ✗ | the row is closed and lacks the key, or the key is `absent` | sound |
| `likely-missing` ⚠~ | not present, and only mutators keep the row open | optimistic |
| `input` ◌ | not present, and the row is open through an input: becomes a schema entry | — |
| `datakey` | an unknown key into a closed record | heuristic |
| `map`, `unknown`, `symbolic` | a dict keyed by data; nothing known; a summary placeholder's row (decided at instantiation) | — |

Over the objects a name may hold: all proven gives proven, all missing gives missing, and
anything else is `possibly`, with the per-object breakdown. A verdict carries its call chain
(`via L96>L85`), since verdicts are per call context. Reads that cannot fail (`.get`, `in`,
`getattr` with a default) give no verdict. Duplicates per object, key, tier and call context
are reported once.

**Names.** A read of a variable bound on some paths only is `possibly` in the `name`
namespace (`name['x'] possibly missing`), and may raise `NameError` (`UnboundLocalError` in a
function) from the state before the read. After the read the name is bound on that path. A local
that no path binds is `missing`, and the read ends the path. Module names use the module's
attribute row, locals `State::unbound`, and a recurring test restores a branch's bound names as
it restores rows (§5.1). Narrowing changes a name's value, never whether it is bound.

## 11. Library models

Models (`models/`) are registered by qualified name (`Registry`). A model receives the call's
arguments and the state. It returns a value, may change the state, record requirements and
raises, and call back into user code (`key=` functions, validators, handlers). An
unmodelled function of a modelled library is an unknown call (§6.5).

### 11.1 Standard library

Exact on literal values wherever CPython's result is decided (`pyvalues.rs`, `pytext.rs`,
`pynum.rs`, `chrono.rs`), unit-tested against CPython:

| area | modelled |
| --- | --- |
| builtins | `len`, `sum`, `min` / `max` / `sorted` (keys, `__lt__`), `any` / `all`, `enumerate` / `zip` / `map` / `filter` / `reversed` / `range`, conversions, `isinstance` / `hasattr` / `getattr` / `setattr`, `vars`, `iter` / `next`, `divmod`, `round`, `abs`, `chr` / `ord` |
| str / bytes / dict / list / set | all methods on literals: `format`, `%`, format specs, f-strings; dict views, order, `fromkeys`, `setdefault`, `update`, `pop`, `\|` |
| collections | `Counter`, `defaultdict`, `OrderedDict`, `deque`, `ChainMap`, `namedtuple`, `UserDict` |
| itertools / functools / operator / heapq / bisect | all of itertools on known items; `partial`, `reduce`, `wraps`, `lru_cache`, `cached_property`, `singledispatch`, `total_ordering`; `itemgetter` / `attrgetter` |
| dataclasses / enum / typing | fields, defaults, `replace`, `asdict` / `astuple`; Enum / IntEnum / StrEnum / Flag; TypedDict, NamedTuple |
| text | `re` (a backtracking engine with Python's syntax: Pattern / Match objects, groups, `sub` with callbacks), `string.Template`, `textwrap`, `shlex`, `html`, `fnmatch`, `base64` / `binascii`, `urllib.parse` |
| data formats | `json` (exact `dumps` text; `loads` of literals, with CPython's `JSONDecodeError` messages and positions; `object_hook`), `csv` (readers / writers, dialects, `StringIO`), `configparser` (INI simulated), `sqlite3` (in-memory SQL simulated: CREATE / INSERT / SELECT with expressions, aggregates, GROUP BY, ORDER BY, LIMIT, UPDATE, DELETE, `rowcount`) |
| numbers / time | `math`, `statistics`, `decimal` (contexts, `quantize`), `fractions`, `datetime` / `date` / `time` / `timedelta` (arithmetic, `strftime` / `strptime`, `isoformat`) |
| os / process | `os.environ` (an input; its stores and deletes tracked), `os.path`, `pathlib` pure paths, `argparse` (parse of known argv), `contextlib`, `threading` / `concurrent.futures` (targets run, futures hold results), `copy`, `logging` |

### 11.2 pandas

A frame's row is its **columns** (tuples for MultiIndex columns). Beside it, `Meta` slots keep
what literal data tells: the index levels, row labels and count, and per column its literal
values and dtype.

A **Series** whose index matters (a column, a frame's row, a reduction, a grouped aggregate) is
a record of its labels.

| call | columns |
| --- | --- |
| `read_csv`, `read_json`, `read_excel`, … | open: an input, so its columns become a schema; `usecols=` / `names=` closed |
| `pd.DataFrame(records / dict / rows)` | the keys, definite when the record count is known |
| `df[[…]]`, `loc`, `iloc`, `filter`, `select_dtypes`, `reindex` | projection in order |
| masks, `query`, `head`, `drop_duplicates`, sorts, element-wise ops | the same columns, fewer rows |
| `df[k] = v`, `assign`, `insert`, `eval("c = …")` | store |
| `drop`, `pop`, `rename`, `add_prefix`, `df.columns = …` | kill or rename (renamed-away names are absent) |
| `set_index`, `reset_index`, `T` | index levels ↔ columns |
| `merge`, `join` | per column, by which side may have it: suffixes on clashes, keys once; an open side never provides a plain name the other side surely has |
| `concat`, `groupby(…).agg / size / transform / apply`, `rolling`, `resample` | unions; aggregates with `(col, func)` columns; group keys first with `as_index=False` |
| `pivot`, `pivot_table`, `crosstab`, `get_dummies`, `melt`, `stack`, `unstack` | from the literal values of the column that becomes the columns, else open |
| `iterrows`, `itertuples`, `apply(axis=1)`, `to_dict`, `to_json` | rows as records with the frame's columns |

A pandas call on a parameter inside a function is decided per call site (a `concrete` event,
§6.2).

### 11.3 pydantic and FastAPI

**pydantic** (v2, with the v1 spellings):
* **Model instances.** Their attributes are the fields: closed, or open with
  `extra="allow"`.
* **Construction** (`Model(...)`, `model_validate`) reads each field by alias. A missing
  required field is a `ValidationError`, and on an open input a requirement.
* **Validation** follows the annotation: lax coercion of scalars, `Literal` / Enum options,
  nested models (whose errors join the outer list under the field's path), lists and dicts of
  them, discriminated unions, `Field` constraints, container types (a `str` for `list[str]` is
  `list_type`).
* **Validators** (field / model, before / after / wrap) run as user code. Defaults are not
  validated.
* **Dumps** (`model_dump`, `model_dump_json`, include / exclude / `exclude_none` /
  `exclude_unset` / `by_alias`, computed fields, serializers) follow their options.
* **Other API:** `TypeAdapter`, `RootModel`, generics, pydantic dataclasses, `BaseSettings`
  (from `os.environ`).
* **Errors:** `ValidationError.errors()` holds each error's `loc`, `type` and `msg`.

**FastAPI:**
* **Routes** (apps, routers, prefixes). Each handler is an entry point with an open request,
  and runs concretely for every `TestClient` call.
* **Parameters:** path, query, header, cookie and body, converted by their annotations (a value
  no conversion accepts is a 422).
* **Request handling:** dependencies (cached per request, `yield`), middlewares, exception
  handlers, `HTTPException`, status codes, `response_model`, and the JSON the client reads back.
* **Escaping exceptions:** an exception the app doesn't answer, including a `ValidationError`
  raised by the handler's own code, propagates out of the `TestClient` call, as it does at
  runtime.

### 11.4 Tensors: PyTorch and numpy

In a program that imports torch or numpy, a tensor's value is its **shape**: a list of
symbolic dims, possibly with a rest of unknown length, encoded in a `Data` label (`shape.rs`).

* **Dims** are integer polynomials over atoms: `x.shape[i]` of an unknown named `x`, named ints
  (`n`, `self.n_embd`), `a // k`, `a % k`, `max`, `min`. Two dims are equal when their
  difference is 0 and different when it is a nonzero constant; otherwise the question is
  undecided.
* **A rest** is a slice of one unknown's shape (`*x.shape[:-1]`), a broadcast of rests, or `*`.
* **Operations check their conditions.** A condition decided false is a **shape error**: the
  operation raises and the path ends. An undecided one is a **requirement** at that line
  (`x.shape[-1] == 512`, `x.dim() == 3`), and from there on a **fact** on the path (`State.tf`).
  `assert` and `if` tests on dims give facts too.
* **Coverage:**
  * creation; elementwise ops with broadcasting; `matmul` / `bmm` / `einsum`;
  * shape ops (`view` with `-1`, `permute`, `transpose`, `squeeze`, `flatten`, `expand`,
    `repeat`, `chunk` / `split`, `cat` / `stack`); reductions; indexing;
  * `nn` layers (Linear, Embedding, norms, convolutions, pooling, RNN / LSTM / GRU, attention,
    Transformer layers, containers) and `torch.nn.functional`;
  * user `nn.Module` subclasses (calling one runs `forward`).

  dtype and device are not modelled.
* **numpy** uses the same shapes, with numpy's semantics where they differ (`transpose`
  permutes, `size` is an attribute, `cumsum` flattens). The fields of a structured array are
  keys.

## 12. Outputs and interfaces

* **Text report** (`rowan FILE`): diagnostics (verdicts other than proven), tensor-shape errors
  and requirements, inferred schemas, function results per call context, module-level values
  at exit, notes (unknown calls).
* **`--json`**: one JSON object per line, `kind` = `verdict` (site, key, namespace, tier,
  object, call chain, note), `schema` (source, key, always / sometimes, sites), `summary`.
* **`--facts`**: the document the editor integration reads: per name and line the values
  (text and structure, with the row sets `present`, `definite`, `likely`, `absent`, `closed`,
  `sources`, `mutators`), verdicts, schemas, shape errors.
* **`rowan serve`**: JSON lines over stdio.
  * `{"id": 1, "cmd": "analyse", "path": "/abs/file.py"}` answers with the `--facts` document
    under `result`, plus `cached`, `ms` and `deps` (the modules it loaded).
  * A result is reused while the file and every module it loaded are unchanged (content
    hashes).
  * `{"cmd": "changed", "paths": […]}` drops the results that depend on those files;
    `{"cmd": "shutdown"}` exits.
* **Imports.** Local modules and packages are followed (relative imports, `pkg/__init__.py`).
  Installed packages are modelled or unknown, unless followed with `--follow PKG` (their
  source from site-packages; no interpreter runs).

## 13. Soundness and limits

The **sound reading** is the `proven`, `possibly` and `missing` tiers, the `definite` /
`present` / `absent` sets of rows and the values, relative to:

* the models: a model that is wrong about a library is wrong in rowan;
* annotations, for values whose class is unknown: an entry point's parameter `x: Foo` is a
  `Foo` (a caller outside the program could pass anything);
* Python's dynamic features: `exec` / `eval` of non-literal code, `setattr` and `getattr` with
  unknown names, metaclass behaviour, and monkey-patching are unknown or ignored.

The **optimistic reading** (`likely`, `likely-missing`) assumes unknown code did not remove
keys. It is labelled as such in every output.

Imprecision rowan accepts (sound, but less than exact):
* lengths join to the shorter known prefix, so a list of length 1 or 2 has an unknown length;
* a slice by an unknown bound has no positions;
* pandas values per row are not simulated;
* `while` loops with unknown conditions join their iterations;
* mutual recursion widens after three rounds;
* a store through a name holding several objects is weak on each;
* runs cut by a deadline lose precision.

The open bugs and planned work are in `SUGGESTED_CHANGES.md`.
