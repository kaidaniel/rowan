# AGENTS.md: working on rowan

`README.md` says what rowan does for a user; this file is for working on the engine (how it works, the owner's conventions, tests, switches, layout), true at the last commit; details are in doc comments.

## How rowan works

rowan is a whole-program **abstract interpreter** for Python (Rust, `rowan/`). It parses with
ruff (private names mangled as CPython does) and walks the syntax tree, running the program on
abstract values. Imports run as Python runs them (`TYPE_CHECKING` False, `import *`, namespaces).

* **Values** (`Value`, `domain.rs`) are a set of object ids plus a scalar part (`KeyVal`): a
  literal, up to 64 literals, the keys of some objects (with the keys they had then, `KeysOf::then`: a key they lose later may still be one; kept in a list's elements: a closed record's keys then, `keys_now`), or `Data(label)`, an unknown named by
  where it comes from (`row["id"]`, `x.shape[0]`). A label can also hold a tensor shape, or a
  builtin type (`typed_label`: `int(?)`; from annotations in general runs, builtins (`result_type`; a range's items, `enumerate`'s index, `os.environ`'s values, `read_text`, a str's items), operators; the keys of some objects: `State::keys_type`, from typed stores' sources `store@L4:str`). Joined with None, the type or None (`optional_label`: `None | int(?)`; a test that removes None, `kv_not_none`, leaves the type). Joined with literals, an operator's (`expr:`) loses its name (`max(1, n - 1)`: `int(?)`); the label still covers the value, and `Data`'s hint keeps the literals (up to 8, then saturated: `hint_sat`) as ones it may be (`8080 | ?`; code ignoring it stays sound): operators, `try_pure` calls (`pure_hint`) and narrowing carry them (`split_hinted`; also literals beside unknown objects).
* **Objects** (`Obj`, `interp/heap.rs`) are identified by allocation site and context (the call
  chain, 2 deep). A *single* object stands for one runtime object: stores into it are strong.
  Allocating a site again while its earlier object is live gives a variant (up to 8), then a
  *merged* object (stores only join); past 256 objects a site's summary (calls the main file's own code makes: 16 more, `MAIN_CLONES`). An instance a call makes where the object is a summary is a strong *recent* one per call depth until the call returns or a raise leaves it (`call_recent`), then folded into the summary (nothing holding it: dropped). Garbage collection at returns, loop and try exits drops
  the state of objects made since (`Obj::born`, renewed by a strong re-allocation) nothing reaches
  (side tables' young targets: a remembered set by age, `Heap::push_copy_roots`).
  An open row's unread key holds a *placeholder* per object and key (`path_placeholder_value`, 3 deep); a
  read on more than 8 objects (`wide_read`) makes one per key there (weak; a library object's attribute: one unknown per name).
* **State** (`State`, `state.rs`) is one point on one path: names → values, and per object
  its **rows** (`(object, namespace)` → `Row`), slots (the value under each key), elements (scalars in
  `elems`, objects in the `Elem` slot: written together, `add_elem` / `set_positions`; missing: unknown),
  lengths (one writer, `set_lens`; a tensor dim after `T` rounds of one append each, `sym_lens`),
  and the requirements recorded so far. The maps are persistent Patricia tries (`layered.rs`): a copy is O(1), a join or diff costs what two states differ in; joins keep
  either side's equal subtrees and a write of the value already there copies nothing (`MapValue`: a row only if the same one), so states stay shared.
* **The analyzer** (`Analyzer`, `interp/mod.rs`) is components with private state, one file each
  (`interp/README.md`); models, the report and the driver see the `Heap`, their `m` and `api.rs`. Its doc sorts
  them for a call memo: running context (a key), state changed outside `State`, output, caches.
* **Rows** are the key-set model: `present` (may be there) ⊇ `likely` (there unless unknown
  code removed it) ⊇ `definite` (there on every path), plus `absent`, open `sources` (inputs),
  and `mutators` (unknown calls). A row without sources or mutators is *closed*; a successful read narrows the key in.
* **Statements.** A test is evaluated once into the states where it is true and false
  (`split`, `narrow.rs`: narrowing reads its parts' values, `traced`); both branches run, join;
  a pure test's branch rows come back when it recurs (`CondFact`: tests a function repeats, until a write).
  Loops over at most 16 known items are unrolled (comprehensions calling nothing per item: 64; items each one distinct object: 256, `distinct_objects`, else every round runs the body once per object);
  others run to a fixpoint (40 rounds, then widened; verdicts and hover values are the last round's), making
  summaries; a target bound to one of the records a summary (or an input's element) stands for
  is a strong *view* per iteration (`bind_views`; `link_view`: changes join both ways). A
  running `try` (`TryCtx`) records the state at every point of its body and callees that may
  raise (a raise, a failing read, a model's raise, any unknown call: class `any_exc`; with the try's frame's names and
  possibly-unbound ones), joined per class (an object a raise leaves half-built, also in a with body, is dropped there unless this path holds it, `drop_unreached`);
  handlers start from the raises they may catch (binding those of their classes), the rest go on through the `finally` to the
  enclosing try. `with` is a `try` whose `finally` is `__exit__`; `finally` runs on every way out.
* **Calls** (`calls.rs`): every call of user code runs the callee's body at the call site (its defaults what `__defaults__` / `__kwdefaults__` hold once read or set), in a
  context of the innermost `chain` (2) call sites. **Budget:** each function code (all closures
  of one `def`) may run 4,096 statements in all (`budget`); past that its calls are unknown
  calls that may also change what its code reaches (`effects.rs`, by `flows.rs`'s reading of
  function texts: globals, cells, defaults and arguments whose value, or what it holds, the code
  may change or give away (to the program's functions: as they use it; to library functions that only read, given no code: nothing, `read_only_library`; a dict, list or set changed
  in place or unpacked (`{**d}`): its items; a new container of its items (`dict(d)`, `list(xs)`: `Level::Copy`) changed in place changes none of them, `d["k"]` is its part under k (`Level::Part`: that slot alone); one its code may remove items of, `del d[k]`, `pop`: those keys, any if not named, not `likely`, `Flags::removes`), a local aliasing what it is bound to; a function argument its code only keeps where the program reads it back (a dict a global, class, module or the receiver holds, under a literal key or its own `__name__`, `setdefault` too; a global list, `append`; through a helper that does; the result): put there, not walked, `Flags::kept`, `KeptPlace`; one only read: not walked; class attributes by name, rebound
  where a store's base may be the class (a call's result: the classes it may return, `result_classes`); a receiver by the names its code uses unless it escapes,
  `receiver_effects`; what its code stores on it on every way it completes, through `super()` too, is there after, `must_stores`; one call for several receivers; a parameter a `return` gives back itself, `return p if c else q`: the argument may be the result, `returns_params`), and the function gets a reporting run (open
  parameters) for the calls that did not run. Entry runs are not counted (one per function): work is bounded by functions × budget.
  * **Recursion:** the inner call uses the running activation's partial result; the activation reruns from the heaps of its recursive calls too (`Recursion`), 8 rounds, then widened. An activation is a call site in its context (4 deep, then the innermost); with literal arguments, its place, receiver and literals (40 deep); a call of itself with nothing but the innermost one's arguments or data from outside: the innermost past one level (`recursion_without_progress`).
  * **Result summaries** (`summary.rs`): a call that does not run gives back what its function returns for any call, as a
    *shape* (literals, typed unknowns; per record born in the run its keys, slots, elements, length, class). At the main
    module's call sites the records are made anew there (`materialize`: up to 16 records, 4 deep; the rest the call's unknown
    result); elsewhere its scalars by type only (`int(?)`: records there cost every later walk and join). An object the run was
    given, read from elsewhere, or stored where the program reads it back (`escaped`: the receiver, a global, unknown code's
    object) is `any`. Shapes come from a *source*: an analyzer of its own (`summary_source`, made at the first skipped call in the main module, also in `rowan serve`'s runs on demand; until then a call elsewhere stays unknown: the module's code with every call skipped, set up for runs on demand, in which every call after a function's first is skipped, `Budget::cap`), which runs a
    function for any call when a call of it is first skipped (`shape_for`; its thread marks swapped in, `ThreadState`; a quarter of the deadline in all); keyed by the `def`'s address (the parse cache keeps it); packages are not followed there.
    A getter (`return self._link`, `return p.a.b`) is the attribute read at the call site where that runs no program code (`getter_result`).
  * **Unknown values** are not taken for the program's instances: their methods are unknown calls (a copy, `x.copy()`, is as unknown); an attribute stored on one on some paths only is what it had on the others. An instance unknown code changed may hold a name its class has (not a method or class); where that code may have removed an attribute, the class's `__getattr__` there is a call that does not run.
  * **Unknown calls** may change everything they reach through what they are given, at any depth:
    records get a mutator, slots may hold anything (one shared unknown object, which no store
    changes: a value stored into it goes to unknown code, `escape`), lists and sets any items
    and length (NamedTuples, patterns, matches, paths, frozensets, enum members' name and value
    cannot change); the program's functions among it may run, a module's too (`follow_code`: their code's closure, cached per roots until a name it read unbound is bound or a module walked gains a name, in rounds that resume one walk, each applying what it gained; a class
    attribute's flags applied again as they grow; the same code followed with nothing it read or changed changed since changes nothing, `Havoc::follows`). A walk from the same
    values (by any unknown code) redoes only what changed (`Havoc`'s memo; the tests of what it met no longer hold); the dunders of the program's instances it meets that may change
    something run as code that does not (`changing_dunders`; a skipped call follows their classes), and their `__call__`, a class's it met `__init__` / `__new__` (it may call them: `construct_dunders`); a generator it meets may be consumed (`unknown_runs_generators`: its code followed, what it was given changed). They return an unknown and may raise anything; its
    items may be those of the mappings it was given itself (not `**d`'s, the callable's own, an `int`'s; open through the call, not
    through a value read under an input's key: `unknown_result_rows`). `a + b` of unknowns: either's elements. Library functions
    known to have no effect are `noeffect` (`ALLOWLIST_PREFIXES`) unless they change their arguments (`random.shuffle`) or get
    what they may call (the program's functions, classes, callable instances; a library function or bound method where they call what they are given, `CALLS_GIVEN`: `operator.call(d.setdefault, …)`; not `inspect.getdoc` & co., which only look at them: `LOOKS_AT_CODE`; given its instances, their changing dunders may run: `library_dunders`; `random.choice` / `sample` give back the items); several callees: `alternatives` (unknown values: one unknown call; a method on several receivers: one call
    per function when it would not run, or on demand, and one for its merged receivers; those none of which runs — out of budget, past on-demand work or the deadline — one `skipped_calls`, also a dunder's on several receivers, `run_dunder`; more than 64 of the program's functions, `PICK_MAX`: one `skipped_calls`).
  * **Builtins** run the dunders Python calls for them (`builtin_dunder`: what `len`, `hash`, `int`, `float`, `str`, `repr` give is of their type, `kv_of_type`); one without a model is unknown code; `hasattr` of the program's function: true for what was stored on it on every path. A TypeError
    may come from those taken to fail only on wrong types, and from hashing a list, dict, set, keys / items view, or an instance whose class
    sets `__hash__ = None` or defines `__eq__` alone (`check_hashable`). Models, operators and subscripts on literals beside objects
    (`"a" if c else [1]`) run on each part (`split_mixed`); beside an unknown object, which may be them, a model gets no literals, and a key, a position, a `del` or an `==` test none (`opaque_key`); `"%s" % obj` runs `__str__` (`%r`: `__repr__`); None beside them raises where Python rejects it (operators with literals or builtin containers, calls, subscripts, attributes, `len`). A name an imported module does not bind, a module's attribute it does not define, one a function was not given, may raise there (the path goes on).
  * **Entry points, general runs:** functions the main run does not call (also those only entry
    runs call) run with open parameters (`param F.x` schemas), from the module's end; a called
    function gets a requirement-only run so too. A *general run* (`Entries::general`: hover values
    for any call, apart from the program's calls, `Fact`) starts from `after_any_code`: the module's
    end after any of the program's functions ran (`follow_code` over all, a result as its callers
    use it, `caller_returns`; instances by name, `methods_may_have_run`), joined with the heaps at
    module code's calls (`note_call`); parameters as annotated (`annotations.rs`: `list[dict[str, int]]`'s parts too, a dict's keys iterated, `Heap::ann`; a dataclass's, NamedTuple's, TypedDict's or pydantic model's instance has its fields, bases' too, each as annotated, a part annotated with one such an instance), a method on any instance (on demand, of a subclass past 64
    of them: any attributes; `__init__` of a class without subclasses on a new one, `new_instance`), a classmethod on the class or a subclass. Facts on: a facts-only general run where needed.
* **Generators** run whole at their first consumption (`force_gen`; `iter()` gives one back, running nothing; run on some of the paths joined only: again, its effects joined in), in the creating call's context; a consumer of item
  *k* sees the effects up to yield *k* (`GenEffects`; at a position not known, or one object for several runs, some yield's;
  joined in where the consumer changed them). If code between items (`note_resume`; unknown code: a key it made
  there-unless-removed too; deletes) changes what the generator reads after a yield (`Gens::watch`, also after the call
  that started it while held, or a global holds it), it runs again with that changed at each yield, and the changes reachable from it, through classes too (`with_gen_restart`; a `for`'s list is read at each step, `ForWatch::reads`).
  `@contextmanager` bodies run at yields. A `for` over a user iterator (`__iter__` returns self) calls `__next__` each round (`for_user_iterator`); over a one-shot iterator its body names (`next(it)` inside), each round takes the item where it stands then (`for_stepping`).
  **One-shot iterators** (generators, `iter`, `map`, `filter`, `zip`, `enumerate`, `reversed`; `iterator`) have a cursor from creation (`start_iterator`); what takes items (loops, models, pure calls, unpacking, `in`, `next`) takes them from it (`positions_left`, `items_from_cursor`) and moves it; no `len`, no subscripts. One over a list, dict, set or deque is *live* (`Tables::live_iters`, also `islice`, `chain`, `zip` & co.): where its containers changed since (`\u{1}srcfp`, hashes of their facts), it takes its items again (`sync_live`: `refill_live`, or the model again; a reversed list's from its first index; a generator expression's: any), and a dict's, set's or deque's whose keys changed raises RuntimeError (surely when its size surely differs); used up, it stays so.
* **Requirements and schemas:** reading a key from a row open through an input records a
  requirement on that input: `always` (every path), `sometimes`; `only_if`: tests of the
  function's parameters as passed that all its readers passed (`State::path`, `guards.rs`). A
  read guarded by a `KeyError` handler counts (a frame column read in a `try` is a probe: not; a catch-all, `except Exception:`, guards only
  reads in its own function: `is_guarded`; a caller's still gets the raise, `catches`); one on one of several objects needs each one's only sometimes (an element's: per element).
  A general run's on its own parameters are a hover's contract (`Findings::contracts`); a parameter's fact at its `def` keeps what each call passed (`Fact::passed`, checked by its run's verdicts).
* **Verdicts:** every read, `del` and `pop` gets a tier per object from its row (`classify`): `proven`, `possibly`,
  `missing`, `input` are sound; `likely` / `likely-missing` optimistic (unknown code removed nothing). In one of
  several functions a value picks (`CONVERTERS[k](x)`, `k` unknown: `cx.picked`) a `missing` is `possibly`. A `map` (a key of a record filled under keys not known: what its `*` slot holds, also where a path stored the key) is not reported alone; beside other objects it may fail.
* **Models** (`models/`, by qualified name in `Registry`; free functions of `&mut Analyzer` that see the `Heap`, their
  `models::Tables` and the interpreter's public methods) compute CPython's result on known values (str / bytes, formatting,
  json, re, datetime, decimal, sqlite, csv, files: `open()` reads str / bytes, a path of unknown text, a Counter's counts; an `lru_cache` / `cache` wrapper a new function, `fv`'s attributes copied, `__wrapped__` it (`memo_wrapper`), a call with known arguments a hit once cached, unless the cache is bounded, else a hit or a miss: `memo_call`; `update_wrapper` as `wraps`…), and simulate pandas columns (an unknown value's mask / column-list / `.loc`
  selection holds its columns, its `drop(columns=…)` the others; a Series iterates its values, Python scalars; a cell
  is an object column's value, a number's unknown: numpy's; literal data's values in row order, `cells`, until its values or labels change; a frame's or Series' truth raises), pydantic / FastAPI validation and routing, and torch / numpy shapes (dims as polynomials; an undecided condition is a requirement and then a fact on the path; in torch modules `int` / `Tensor` annotations hold; a module-level unknown number names its dims, a module-level tensor's unknown dims are one value each).
* **Deadline:** a safety net (`ms`, 10 s; time in followed packages' code not counted: theirs is a share of its own): past it user calls are unknown, running ones stop, entry runs left out (`cut`); the budget bounds the work.
  **On demand** (`on_demand`, `general_run_of`): from `after_any_code`, a hovered function's general run with
  verdicts (fresh budget; past `fanout` (4) budgets of work its callees' calls are unknown, `callees_cut`, several callees one `skipped_calls`,
  classes among them their instances made there; every run starts from the module's end, `demand_start`). `rowan serve` starts the whole run and the module code's
  (`__main__` block's calls skipped) together; the latter answers unless the former completes within `EARLY` (1 s), whose
  document then follows as an event. With `--follow` all this runs without following (the first answer as fast) and a whole run following starts too, whose document replaces it when it completes (`followed` event; the others' then say nothing). A changed file, a newer `analyse` of it or a `cancel` (the extension's, when a
  request times out) cuts the analyses running for it as the deadline does (`Budget::cancel`, read on its own thread).
* **Installed packages** (`--follow PKG|*`: site-packages of `--site-packages`, `$VIRTUAL_ENV`, a `.venv` above the file; not modelled ones; `library.rs`) run as the program's code, with per-function budgets; each entry of the program's code into them (an import, a call) also gets an allowance of work (`fanout` budgets, definitions not counted),
  past which its calls do not run (`skipped_call`; module code goes on: an import's names are known, its expensive calls unknown; such a call whose code, some of it a library's, has a closure past `SHARED_PAST` (128) walk visits, alone or in a later round, follows one closure all of them share, `LibClosure` (`libclosure.rs`): walked once, extended by what each call adds, applied again only to what it gained and what the state changed since it was last applied (a trie diff); it may change more, never less). All entries share the *library's share* (`LIB_SHARE`: 64 allowances of work, walks counted; half the deadline), apart from the program's budgets and deadline: past it the running entry stops (every statement, `cut_body`; every call) and is undone (the state before it, its modules forgotten, `forget_since`) and is what it is without following (an import an unknown module, a call one that does not run); then no library code runs. A class whose followed base's `__init_subclass__` does not complete has an open base. But a call of *inert* code does (`inert::inert`: straight-line, it makes functions and stores attributes, `functools.wraps` decorators; also past a run on demand's work). Their own verdicts are not reported. **Deferred definitions** (`deferred.rs`): `@d… def f` in an imported module whose decorators are all *plain* inert (one parameter, reads only names bound at the `def` and what every function has, calls only `wraps` / `update_wrapper` / `setattr` / `getattr` with a default / `hasattr` on its own values, gives back a function) binds a placeholder; the decorators run when `f` is first read on a path (a name, a module's attribute, a call or attribute of the value, `vars` / `globals()` / `import *` into the main module / `getattr` by an unknown name), from their names' bindings at the `def`, once per path (`\u{1}forced`, a key of its row: run on some of the paths joined only, it runs again on a copy, joined in). Sound because such code changes only `f` (fresh) and what it makes, cannot raise, shares no state with others; `from m import f` in a module's code binds the placeholder; unknown code reaching it may run the decorators' code and `f`'s. Not in the main module (what the editor shows); registries, raising or reading decorators run at the `def`.
* **Soundness** holds relative to the models and to annotations (of values whose class is unknown;
  a general run's `x: int` is an int, not a subclass overriding int's operators; an unknown the program gives a dataclass's, NamedTuple's or TypedDict's field annotated `str` / `Optional[int]` is one, `field_as_annotated`, `float` only validated: an int passes; a pydantic field's validated unknown is of its type). `getattr` by a name not known reads every attribute
  of the program's objects, quietly (known in part, an f-string's or a name a template makes, `"_on_%s" % m` also through a local: those whose names fit, `Payload::Affix`, `template_affixes`; a container's, or past a wide
  read: one method per call site, unknown code on them when called); `exec`, `setattr` with unknown names on a class, metaclasses and
  monkey-patching (rebinding a function or class too; a module's function the program calls only by name has no other callers) are unknown or ignored;
  unknown code does not shadow a class attribute on an instance of a library-based class; an unknown beside objects is no instance
  of the program's or a library's classes (`isinstance`: a builtin's, by its type; it decides and narrows literals by type, typed unknowns against builtin classes, `int(?)` not against `bool`, `kv_isinstance_narrowed`); a generator expression runs when made (a container it iterates changed before its items are taken: they are not known). List lengths are tracked up to 15 (a few per path: each one's positions;
  `xs[:n]` keeps xs's). An object a path has no facts of reads as unknown (`read_row`). A set's items, and a dict's whose insertion order depends on the path, have no order: each is any of them for every order-dependent use (`Iteration::items`; `any_order_items` for sets, `sorted`, comprehensions).

## Working conventions (the owner's standing instructions)

* Python runs only through `uv run --no-project python` (3.14 pinned), for the test harnesses.
* **New test cases are specifications.** Write the expected rows and verdicts by hand from the program text first, then make the engine produce them. Never derive an expectation from engine output. When an expectation turns out wrong, fix it with a comment saying why, keeping the line count. Value analysis matters more than bug-finding verdicts.
* **Hypotheses need a repro.** For a suspected bug or slowdown, a small program showing it first (a golden case; `#@work` / `#@tabulations` bounds for cost), fix it, then measure.
* **Soundness first.** Where the runtime oracle and this file disagree, the oracle wins. Golden cases matter; snapshot changes are fine when sound.
* **Commits.** Commit when a round of work is complete and golden, snapshots and `cargo test` pass; never commit a failing state. Review snapshot diffs before `--update`.
* **Plans.** When a request mixes several things, write the plan first, then work it in order. Keep this file under 200 lines and up to date with every change.
* **Output is for Python engineers.** The text report, hovers and diagnostics use words and Python access paths (`row["id"]`, `items[*].sku`), never engine terms (row, tier, source, placeholder) or internal site labels (`report::site_text` names unknown code and a model's inputs by the call as written, with its file when another). A hover (`renderHover`): `name = value` (an open record's keys only possible, of values not known, are its `**?`) at the hovered place on the line (`(parameter) int(?)`; a target: the value assigned; facts per line, column, name: `factsAt`), then Requirements (of parameters: their function's contract, also in its body; inputs read on the line), In Calls (`on line 83 in dp/api.py` when in another file; a site the main run gives: not again from an entry run), Allocation Sites (not one object made by the name's own assignment), Notes (only what is used above, `list[x]` / `dict[k, v]` notation too; last, a cut analysis). `vscode-rowan/test.js` checks real hovers.

## Commands

```
cd rowan && cargo build --release && cargo test --release --lib && cd ..
uv run --no-project python tests/run_golden.py      # all suites, ~5 s; paths select cases; -v; --pending [--prune]: cases in tests/*/PENDING
uv run --no-project python tests/run_snapshots.py   # --update only after reviewing the diff
uv run --no-project --with pandas --with pydantic --with pydantic-settings --with fastapi --with httpx --with numpy python tests/oracle.py [--strict|--lenient] [-v] [-j N] [case…]  # vs real runs, ~1.5 min
uv run --no-project --with torch --with numpy python tests/torch_oracle.py [case…]
uv run --no-project python tests/fuzz.py [KIND…] [-n N] [--seed S] [--shrink]  # random programs vs CPython, also at budget=0; -n 50: ~5 s
cd vscode-rowan && node test.js && npm run package  # hovers; the .vsix (test_package.js X.vsix TARGET)
uv run --no-project python bench/run.py rich rowan/target/release/rowan "$(uv run --no-project python bench/prepare.py rich)"  # also email, stdlib12k, click; pip, timm, sktime, functional
uv run --no-project python bench/hovers.py rowan/target/release/rowan pip  # each function on demand (`rowan hovers F --only NAME`: one); tests/ondemand_check.py [-j N]: vs whole runs; bench/first_hover.py (same arguments, rich): rowan serve's first hover in a file
uv run --no-project python bench/sitepackages.py [-v] [--uncapped]  # installed packages followed (`--follow`): cost and hover precision per scenario, ~10 s (venvs cached in bench/work/); --uncapped: also no allowance (fanout), no bounds
```

Run the oracle on the cases you touched, in full before a commit that changes models. CI (`ci.yml`, every push and PR): build, `cargo test`, golden (`--pending` informational), snapshots,
`fuzz.py -n 50` (seed 1; a per-run seed informational), `ondemand_check.py -j 4`, `node test.js` (Linux, macOS); `cargo clippy -- -D clippy::correctness`; both oracles; a packed .vsix per native target. `bench.yml` (not blocking): `run.py` on all eight, `hovers.py` / `first_hover.py` on rich and pip, `sitepackages.py` (no `--uncapped`), the `budget=0` golden count; on main into `history.py`'s charts.

## Tests

* **Golden suites** `tests/{cases,torch,pandas,pydantic,fastapi,stdlib,apps,numpy}/`, ~1200 programs; library suites have `small/`, `models/` (torch `playground/`: the owner's), `PENDING`.
* **Directives** (full rules in `tests/CONVENTIONS.md`, also `tests/{torch,pandas}/`):
  `#@ tier key`, `#@row NAME: present= definite= absent= closed|open`,
  `#@value NAME: exactly= | unknown | includes= | shows=` (`#@general`: for any call), `#@schema src: …`, `#@only src: KEY if TEST | KEY none`, `#@todo …`, `#@argv`, `#@shape` (torch). Cost bounds, by hand:
  `#@objects`, `#@tabulations`, `#@joins`, `#@work`, `#@visits`, `#@havoc <= N`; `#@limits k=v,…` sets the case's `ROWAN_LIMITS`. A directive-only line applies to the code line above.
* **Oracle** (`tests/oracle.py`): reruns each case under CPython (a tracer), checks every directive (schemas by dropping keys), strict on library suites. **Fuzzing** (`tests/fuzz.py`): random programs in 16 areas (lists, dicts, attributes, try, generators, formatting, calls, insertion order, control flow, result summaries and getters, decorators in a second module the program imports, code handed to unknown code (`operator.call`)…; those three also at `budget=6`; positions, keys and names also beside an unknown object: `lx`, `kx`, `ax`); the values, key sets and access outcomes of CPython's runs must be among rowan's claims (`--shrink`: a repro). **Snapshots** (`tests/snapshots/`): `--json` on `examples/etl_report.py`, `tests/fixtures/proto_*.py`: the guard for internals (ids may shift). **Unit tests**: CPython values, the regex engine, tries.

## Debugging switches

* `ROWAN_DEBUG=1`: body runs (`run F at SITE`), skipped calls, entry runs, a `work:` line of counters (statements, units = statements + walk visits (reachability and closure walks) / 16, body runs, objects, row joins, visits, havoc visits; ~25% slower), each library entry's work, the functions doing most (runs, own statements, units and ms of outermost runs). `2`: raises.
* `ROWAN_LIMITS=budget=4096,ms=10000,fanout=4`: the only knobs (golden `#@limits` per case); `ROWAN_FOLLOW=pkg,…` is `--follow`; `ROWAN_TODO=check`. Profiles: the bench workflow's `perf`; locally `xctrace record --template 'Time Profiler'` (symbols: `atos -l` with the binary's slide; `sample` hangs on macOS 27); time inside the guard (`.guard/run /usr/bin/time -l`).

## Layout (`rowan/src/`, ~72k lines: the engine ~29k, the models ~43k)

Maps of the code, file by file, with where to start reading: `rowan/README.md` (the crate, the binary, `py/`), `rowan/src/README.md`
(syntax, domains, `main.rs` / `serve.rs`), `rowan/src/interp/README.md` (the `Analyzer`'s components, how a program runs),
`rowan/src/models/README.md` (what a model is, a file per library). In short: `frontend.rs`, `scopes.rs`, `syntax.rs` parse; `intern.rs`,
`domain.rs`, `source.rs`, `layered.rs`, `state.rs` are the domains; `interp/` interprets (`report.rs`, `render.rs`, `facts.rs`: the output);
`models/` are the libraries; `rowan/py/` a PyO3 binding (does not load on macOS 26); `vscode-rowan/` the editor extension (talks to `rowan serve`).

## Implementation rules that are easy to break

* **Models allocating several objects at one node** must give each its own context (calls: `Node::sub`, a field's factory: `factory_site`).
* **Callees** run with the caller's `state.exits` taken out and restored (y438).
* **GC roots.** A side table keyed by objects goes into `side_edges` (and holds for a recent object's summary too: `Tables::rename_recent`); anything else the analyzer holds (a running with's managers) into `gc_pinned`: else a read gets an empty row (and a list no length).
* **Generator yields** use `alloc_raw` for per-yield states. **Stdlib objects** (`stdobj.<kind>`): Attr slots, `m_stdobj`. **Attributes resolved by class** (`class_resolves`): class, model and `__getattr__` objects have more than their rows show.

## Measured (release build, Apple M-series)

| input | time |
| --- | --- |
| stdlib12k (12k lines) / email (10k) / rich (27k) / click (10k), no deadlines | ~2.7 / ~2.2 / ~5.5 / ~5.7 s; 0.14 / 0.07 / 0.12 / 0.12 GB (`--facts`: ~4.7 / 2.5 / 8.9 / 6.4 s) |
| pip (134k) / timm (132k) / sktime (355k) / functional (213k), no deadlines | ~11.1 / ~0.65 / ~3.2 / ~4.9 s; 0.33 / 0.22 / 0.21 / 0.31 GB |
| installed packages (`bench/sitepackages.py`, s1–s9), `--follow '*'` | 0.37 / 0.20 / 0.53 / 0.19 / 2.9 / 2.0 / 1.2 / 0.44 / 0.26 s; 30 / 29 / 62 / 73 / 401 / 406 / 169 / 79 / 87 MB; facts gained 6 / 32 / 13 / 13 / 6 / 8 / 14 / 6 / 10, lost 0 (none: < 0.01 s, 10 MB); `rowan serve`: first answer as without (13–23 ms), followed document at 0.18–2.2 s |
| on demand (`bench/hovers.py`, every function): rich / pip / timm / sktime | setup (every module) 0.11 / 6.6 / 1.0 / 0.7 s; per function p50 ≤ 5.7 ms, p99 0.12-0.20 s, max 0.46 / 0.51 / 0.41 / 1.3 s; first hover in a file (`bench/first_hover.py`) p50 1.0 / 1.0 / 0.94 / 0.08 s, max 1.2 / 5.2 / 1.2 / 1.0 s; `rowan serve` on each timm model file ≤ 0.5 GB |

## Known problems and next steps

Open items and ideas ruled out are in the pull requests' descriptions (Remaining, Ideas not pursued). Parked: `parked/joined-summaries` (unsound), `slotmap-trie`, `parked/memo-probe`, `parked/suggested-changes` (the retired list of open items).
