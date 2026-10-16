# Suggested changes: avenues not yet explored

Only open items, by topic, each with a repro or pointer. The PRs are the changelog.

## Overseer notes

* **Owner's standing rule:** don't over-index on the very large programs. Never trade speed or
  more than a little precision on the smaller programs (stdlib12k, email, rich, click,
  golden-sized code) for the huge ones; a real soundness fix may cost them speed. Every local
  run goes through `.guard/run`, small ones included.
* **Owner, on AGENTS.md:** it introduces the code base (how it works, conventions, tests,
  layout). Nowhere say "accepted imprecision": every imprecision is a trade-off (more precision
  costs memory, time or complexity), so call it "known" or "known to be hard", name the cost,
  and keep it as an open item here.
* **Owner, on `#@todo`s:** they are evidence, not targets. Don't overfit to the golden
  examples: a change must be a general improvement for real Python, judged on the benchmarks
  and hovers too, not a mechanism shaped to make particular todos pass.
* **Plan.** The owner's 21–24 plan (facts that hold for *any* call of a function, mostly over
  its parameters, shown in hovers and computed on demand) is complete: PRs #21, #22, #25, #26
  (v0.1.21–v0.1.24). Its 2× budget is spent: new work is small-programs-first again. The owner
  will inspect the end state against v0.1.20 and may go back on a step. Each iteration ends
  with a plain explanation for the owner; "already done" / "can't be done, because …" are fine.
  * **Next:** the open soundness bugs (Soundness bugs below, 15 items): repro each as a golden
    case checked by the oracle, fix the real ones, say which are not reachable and why.
  * **Then:** what is left of the editor's tails (Editor below: sktime classmethods
    constructing every subclass, timm's distillation tasks; entry runs whose loop rounds
    explode in objects, timm `adapt_model_from_string`; the whole run in `serve` not cancelled
    when its file changes).
  * **After that:** what is left of the any-call start's precision (Entry runs: stores into global
    containers, attributes methods store, `self` handed to a library, library calls given
    globals; verdicts for any call do not pay yet, measured there); the remaining
    `kv.as_lit()` uses in models; the residual entry-order dependence (stdlib12k, functional);
    summary comprehensions outside torch.

## Soundness bugs

* **getattr by a name not known at all** (`getattr(self, name)`, name from data) is still an
  attribute rowan does not follow (its call is unknown code that reaches nothing). A name
  known in part is matched by its f-string's prefix / suffix (`Payload::Affix`); one built
  otherwise (`"run_" + mode`, `"%s_x" % m`, a template of several holes) is not.
* **A class attribute assigned in skipped code through a call's result** (`make().x = 1`)
  rebinds that attribute of every class the code reaches (by name, as a parameter's store
  does): sound, but a method of that name elsewhere becomes `?`. Through a global's value
  (`REGISTRY[k].x`, `for c in CLASSES`) only the classes that value holds are rebound.
* **Hashing:** an instance of a class that sets `__hash__ = None` without defining `__eq__`
  is taken to hash; dict views (`d.keys()`) as keys are taken to hash (they do not).
* **A recursive function's reporting run past its budget** (z551 under `budget=0`): the read
  at its top is proven in a run from a state where the global key is there, though a deeper
  call reads after a deletion; the skipped recursive call's effects come after the read.
* **Mixed values with None** (`None` beside objects) are not split per part (`split_mixed`
  leaves None out: splitting every Optional doubled model runs); a model given `None | [1]`
  may compute only the list's result where None would raise.

## False ✗ left on the benchmarks (imprecision reported as failure)

* sktime `_delegate.py:40` `getattr(self, self._delegate_name)`: ✗ on the entry-run instances
  of IgnoreX / ForecastByLevel, made by `__init__` with any `forecaster` (None: `_get_delegate`
  before `__post_init__` sets `forecaster_`, a real AttributeError for `IgnoreX(None)`); the
  driver never passes None. An entry run's instance could take its parameters from the
  program's own constructions of the class when there are some.
* functional `hypothesis/_settings.py:1127`: key from a class attribute behind a metaclass.
* stdlib12k `_pydecimal` L534-555 (`value._exp` on `Decimal@L3804` variants, `value[0]` after
  `isinstance(value, (list, tuple))`), pydoc L10840-10863, L3833 `saved_context` in an entry
  run of `__exit__`, argparse L6820 / L6867 / L9103. Repro: lines 1-6392 of `big.py`.
* email `message.py:932-940` `charset[0]` after `isinstance(charset, tuple)`;
  `generator.py:259` after `del msg[...]` on an unknown object. Not reduced yet.

## Precision gaps

### Views, loop records, values in part (were listed as soundness bugs: no unsound repro)
* **Views mirror rows by joining, except a stored key** (`state::link_view`, `mirror_row`): a
  `del` / `pop` / model change through the summary or another view joins the summary's row
  into the item's, losing the item's own keys (`r.pop("d")` after the item's `d` was stored:
  ⚠ where it always succeeds). Repro attempts (pop / del / update then reads in later loops,
  nested loops deleting through another view, a view kept after `break`) all stay sound. An
  operation-level mirror (as `store_key_row` does for a stored key) would keep the keys.
* **Views are flow-insensitive links** (thread-local `VIEWS`): a view escaping its loop stays
  linked (changes keep joining both ways: sound); a view in use when its loop runs again
  refuses a new one (the name keeps the summary: imprecise). A `Value`-level iteration tag
  would tell `prev` from `r` (z513 / z514 `#@todo missing seen`).
* **`df.apply(f, axis=1)`'s row** is the weak per-frame record: `row["c"] = x` then
  `row["c"]` in `f` is ⚠ (a view per call of `f` would make it ✓); deletes in `f` do not
  reach the frame (right: apply gets copies).
* **A method of a value that is a literal or an object** (`x.upper()`, x "dev" or a list):
  the list's missing method gives an unknown instead of an AttributeError path (z548 todo).
* **An environment variable is a str**: `os.environ.get(K, "a")` is an unknown of no type, so
  hashing it in a `try` may raise TypeError (z544 uses `str(...)` to have a typed one).
* **JSON text joined with another literal** (`json.dumps(d) if c else "{}"`) is any text
  (z545 todo): a text label holding several snapshots would keep both parses.

### Calls past their budget, unknown calls (objective: `budget=0` golden count)
* **Skipped calls' effects** (z449, 5 to-dos): argument values (`k` is "b"), an escape into a
  known list, deletes through a global, bound methods held in attributes are not told apart.
  Parameters now go by `flows.rs` flags (read / parts / changed); by attribute names (as the
  receiver) would be the next step; email's FeedParser (a generator over itself) defeats it.
* **Skipped methods' sure stores:** only top-level `self.x = …` before any return count;
  stores in both branches of an `if` or in a completed `try` body could too.
* **Past-budget results are unknown** (z442 ok 0, z447 possibly a / n exactly 4); a pure
  helper (empty footprint in its first runs) could keep a cheaper fallback.
* **Summary comprehensions only in torch modules:** `[Foo() for _ in range(n)]` elsewhere is
  still unrolled into n objects whose methods every later loop runs n times; a rule by cost
  (the first iteration's allocations / statements) could take it for any program, at the price
  of per-index identity (`players[0].x = 1; players[1].x` is `possibly`, z510 L26).
* **Validation loops inside training loops** (p06: `for _ in range(10)` unrolled in each of the
  training loop's rounds: 33 of the 46 model calls): a known-items loop whose target is unused
  could run as a loop when its iterations cost much.
* **Returns are escapes for skipped calls** (`flows.rs`, `return_flags`): the skipped call's
  result is unknown, so what the caller does to it cannot be followed back to a global the
  callee returns. Only the program followed whole (`after_any_code`, `caller_returns`) follows
  module functions' results to their callers. Methods' and properties' results stay escapes
  (`Console.file` returns `NULL_FILE`): callers' uses as `Attr(name)` and what methods do to
  their receiver resolved on the value's class would do; rich's NULL_FILE escapes anyway into
  `LegacyWindowsTerm(self.file)` (`self._file = file`).
* **Stores into a fresh instance are escapes**: `self.box = box` in `__init__` lets the default
  change, as the instance (returned by the constructor) may reach code that changes `.box`'s
  value. Sound tracking needs where the instance goes (rich: it is returned and stored).
* **Arguments stored into the receiver or passed to `super().__init__`** escape (click's
  BadParameter / UsageError: ctx walked at each skipped construction): following `self.a = p`
  (unless the receiver escapes or a method reads `a`) and the callee's parameter flags for
  `super()` / `self.m()` calls. Measured with both taken unsoundly: click -9%.
* **`a, b = x, y`** binds pairwise but still takes the display's parts as kept (changed).
* **Held walks of big tuples** (rich: `get_character_cell_size` returns a part of
  `CELL_WIDTHS`, ~450 tuples walked per skipped `cell_len` when the havoc memo misses): a
  deeper level for parts of parts, or a per-object "nothing mutable below" mark.
* **Unknown library calls given instances**: their dunders / protocol methods are not
  followed; methods reached only by a computed name are not in `code_closure`.
* **Unknown calls of several callees** each run on a copy then join (sktime 7M row joins);
  one call for all halved joins but raised havoc 22% and changed schemas.

### Relations (facts relating values; 22's research)
* Tests qualify only over parameters never rebound and unwritten since the call (keys,
  unknown objects): in-place changes of a parameter void them (sound, imprecise).
* Measured, per benchmark (stdlib12k, email, rich, click, pip, timm, sktime, functional):
  conditional `if`s 77k / 48k / 83k / 73k / 133k / 76k / 44k / 60k; joins where two or more
  names or rows differ (a correlation lost) 17k / 16k / 24k / 13k / 36k / 29k / 6k / 15k;
  branch facts made 12k / 5k / 26k / 5k / 24k / 12k / 1k / 8k but used by a later test of
  the same condition 2 / 0 / 172 / 226 / 44 / 0 / 18 / 16 times; parameter tests 2.6k / 0.6k
  / 4.4k / 4.8k / 5.7k / 4.2k / 1.5k / 4.1k. Path-sensitive states (keeping both sides of
  such joins) would multiply the statements run after each of those thousands of joins.
* **Relations with no consumer yet:** index bounds (`i < len(xs)`): rowan reports no
  IndexError at all (`[1, 2, 3][5]` passes); `x is None` correlated with a flag: an attribute
  read on `None` is not reported (`m = None; if v: m = M()` … `if v: m.log()` says nothing).
  Bounds or None-correlation pay only once those reads get verdicts.
* **Keys of a result present only under a test of the parameters** (`make_user returns
  {"id": …, "role" only if admin}`, and a caller's ⚠ saying "only set if `admin`"): needs
  per-key provenance in rows (the path's tests at each store, intersected at joins); every
  write that adds a key, the models' included, must set or clear it, and it is part of every
  row's equality and hash. Branch facts (`CondFact`) hold it only until the row's next write.
* **"Always if admin"** (the sufficient direction of `only if`): from the branch join alone it
  misses every read in a branch that `return`s; needs the path's tests on the states that
  `return` and a join that keeps a requirement's test when the other side's tests exclude it.
* **`only if` for inputs that are not parameters** (`if "a" in payload:` on `json.loads(…)`):
  the test in the input's own terms (`["a"]`), when the test reads only that input.
* **Tests of a parameter changed in place** (`row.setdefault(…)`, then `if "x" in row`) do not
  qualify (sound); a test of keys the function never wrote could.
* **Counted loops:** a cat whose part is not a name (s157:31), `for x in xs` over a list of a
  symbolic length, `while i < n` counters; rounds of a named int need `n >= 0` (a requirement,
  as torch dims are).

### Entry runs and coverage
* Verdict runs of uncalled functions still take instances made with `__init__`'s defaults
  (their verdicts are a caller passing nothing); with facts on they get a facts-only general
  run (`Run::FactsOnly`): stdlib12k +17% statements in `--facts` mode, ~0 elsewhere. Taking
  the any-argument instance for verdicts too (measured): stdlib12k proven records 3240 ->
  2549, likely 542 -> 1005, +17% work; email proven 621 -> 588; click proven 278 -> 362.
* Typed unknowns: `domain::typed_label` (`\u{2}int\u{2}name`), `Obj::ty` for annotated
  parameters, `Analyzer::value_type` / `as_type`, `models::result_type` for builtins.
  Not typed yet: `float` / `complex` annotations (an int may be given: `int | float` would
  need a union type), return annotations, a typed int's or str's falsy value (`0`, `""`)
  in narrowing, the bare `"bool"` label of comparisons of unknowns (may be an array),
  str methods of untyped unknowns (`"str"` label: bytes methods give bytes).
* Ruled out: running the general run first, at a function's first call, with the per-call
  runs on what is left of its budget (`ROWAN_GENERAL_FIRST` probe): the run starts from the
  module's state at that call, so later globals are missing (pip 34 false `missing`), and
  coverage falls (sites rich 632 -> 208, click 298 -> 112, pip 1456 -> 786, email 457 ->
  315); timm 26 s -> 718 s. A skipped call cannot use the general run's result as its
  summary without mapping the parameters' objects to the arguments (unsound as it is).
* Verdicts, row checks and `param F.x` schemas of entry runs still mean "a call right after
  the module ran" (from the module's end, nothing else run first). Measured (iteration 25,
  after its precision work): every entry run from the any-call state (`after_any_code`
  applied to the state they start from) fails 20 golden cases (expectations that hold for
  the real runs) and costs proven verdicts (numbers in PR #27). The causes, by case: what a
  function stores into a global container is "anything" (`Havoc::edit`: f014 f018 f019 f022
  f023 f024 f028 f031 fm01-fm05 store records of a known shape: the stored value's own flow
  would do), a global given to a library call (p112 `orders.merge(LOOKUP)`, n006
  `np.array(ROWS)`: a library's "keeps nothing it is given" would do, as `noeffect` does for
  the main run; numpy / pandas constructors may share an array's memory), attributes methods
  store (b005, a012), a registry of callbacks (b003, sm04), torch modules' instances (p05).
* `after_any_code`'s precision, left: a method that hands `self` to a library function
  (`self.apply(fn)`) still walks the instance whole; an attribute some method stores may hold
  anything (`methods_may_have_run`: rich `Bar(begin~=0 | begin | ?)`); a store into a part of a
  global (`G[a][b] = v`) walks all G holds (its parts edited in place would do: a level of
  `edits` below `held`); results of methods and properties stay escapes (callers' uses as
  `Attr(name)` resolved on the value's class); a function's results passed through a local
  callee (`f = find; f(k)`) or its module through a local (`m = mod; m.f()`) give the result
  away (`NameUses`); a module imported only inside a function that has not run is not loaded,
  so its code is not followed (nor before: its direct changes of globals). Tried: a parameter
  given on to another program function taking that function's flags (transitively): no
  golden hover value or `budget=0` case changed, rich's schemas shifted.
* The heaps at module code's calls are one join for every function (`calls_heap`): a
  registry filled by decorators at import is "some keys" in every general run (golden: ~65
  hover values, a005 `HANDLERS`, h68 `REGISTRY`). Measured alternatives: a join per function
  (exact, but facts-mode memory 3-6x: rich 1 GB), the join from a function's first call on
  (a suffix of snapshots: pip 793 MB vs 299). A join from the first call with bounded memory
  would need the heap's history kept as diffs, not as versions.
* Objects with one id over time (summary comprehension objects, s155 `Head(q?=…)`): the
  join with earlier heaps makes their attributes "on some paths".
* **What is reported still depends on the order of entry runs** on stdlib12k (reversed:
  ~150 of 1838 sites change tiers, 77 more sites) and functional (37 sites): which calls the
  per-code budget refuses (and so which functions get reporting runs, which callers see
  skipped calls) depends on which runs came first, and flow-insensitive tables (the
  `code_closure` cache, computed in the state it was first asked in) differ. A fresh budget per
  entry run is 7x the work on stdlib12k and still order-dependent; capping an entry run's share
  of a code's budget (B/4) changed little; the closure cache recomputed each time is 2x slower
  on click / pip with the same results in source order. Click, pip, email, rich are
  order-independent now. Repro: permute the `from..n` loop of `analyse_uncalled_range`.
* **Entry runs of a class never made itself** (z451 `input type`): its subclasses' instances
  are the receivers that exist.
* **Non-main modules get no entry runs** (email): code reached only through unknown values is
  not analysed.
* **An entry run's instance keeps `__init__`'s values** for attributes other methods rebind;
  an entry run of `__exit__` / a method does not start from an instance other methods touched.
* **Verdicts of superseded rounds are dropped** (loops, recursion): accesses reached only by a
  callee an early round ran lose their verdicts (stdlib12k: 94 sites).

### Open `#@todo`s by cause (69; iteration 19's triage, 22's relational notes)
Each line: the cause, what would fix it and what that costs, the cases. `ROWAN_TODO=check`
checks them.
* **Frames keep column value sets, not rows** (pandas cells; not relational, none of 20-24: a concrete table): values through groupby / sort /
  `idxmax` / `value_counts` / filters, `df.empty` deciding a path, records into frame columns.
  Needs a concrete table for small literal frames; costs memory and a second frame model.
  apps/b002 L43,44,46,61,74,80; pandas/p091:13, p094:17; fastapi/fm04:124,165; apps/c001
  L242 pivot_a, L244 pivot_b, L258 chars_b, L291 one_kept.
* **Relations between columns** [relational: a column present iff another is absent; columns added together] (`note` absent ⇒ `note_y` there; `update()` adds min, max and
  mean together): needs relational facts between keys; costs a fact store per row.
  pandas/pm04:107, pm14:139.
* **Which branch a value came from is lost at a join** [relational (22 measured the joins that lose a correlation: thousands per benchmark, see the overseer notes): which object a name holds ↔ the keys stored / the column list ↔ the stored column ↔ the record's computed key] (`r = mark(a if c else b)`, a column
  list built on the path that stored the column, a before-validator's copy vs its input, the
  function picked by a key computed from the record): needs path-sensitive states or a
  relational domain; costs exponential paths / a relational join. cases/x347:18,
  z512:14,19; pydantic/y053:61; pandas/pm08:177, pm10:170, p074:19,20.
* **Relations across loop iterations** [relational: `i > 0` ⇒ key stored; `len(out) ≥ 1` ⇔ the loop ran; the item ≠ the previous one] (`i > 0` ⇒ an earlier iteration stored the key; the
  first append of a fixpoint loop is position 0; a list is non-empty iff a loop ran; the
  previous iteration's item is another record): needs loop peeling or a `Value`-level
  iteration tag; costs one more body run per loop (time) / a tag in every value.
  cases/z443:46, z494:29, c28:27, z513:69, z514:140.
* **List lengths that are sums of unknowns** [relational: `len(records) == len(csv) + len(jsonl) + len(log)`, positions by offsets] (alpha's batches cut at a size from os.environ,
  then concatenated: positions lost): needs symbolic lengths outside torch (22's `sym_lens`
  holds tensor dims only: lengths of non-torch lists are named ints, which `range` may make
  negative) and positions by offsets; costs a second length domain. apps/c001 L204 rej0_type, L209 src_counts, L232 by_region_a, L237 by_day_a,
  L249 errors_a.
* **Torch: a loop's joined rank, a run that returned saw the loop run** [relational: `ids` is the
  parameter ⇒ any rank, the round's ⇒ `ids.dim() == 2`] (`ids` open before generate's loop, 2-D
  after a round; MyRNN's stack of no tensors raises, so a returning run had a round): rank facts
  kept at a loop head; costs widening by rank. Counted appends / cats are in (`sym_lens`,
  `counted_cats`); a cat whose part is not a name (s157:31) is not counted. torch/p04:95,145;
  p06:58,59,61,65,68,70,158 (158: `ids` is `[1, _]`, encode past its budget), s157:31.
* **Calls past their budget are unknown code** (not relational; 21 may help: the general run's result as a skipped call's summary, argument values aside) (their results, a generator's items, what they
  delete or keep of a receiver held elsewhere or reached by an argument): needs summaries
  that use argument values; costs the budget's bound on work (time), effects cached per
  argument value. cases/z442:23, z447:22,27, z449:31,49,68,91,113; torch/p06:155.
* **A recursion over an unknown depth to computed keys** [relational: key `"level" + str(d)` ↔ value `d`] (`d["level" + str(depth)] = depth`):
  needs a relation between key and value; costs a relational map domain. cases/z434:38.
* **Entry runs of a class never made itself** (not relational; 21: the general run with an annotation-typed receiver replaces this entry run; its subclasses' instances are the receivers
  that exist): needs entry receivers drawn from the subclasses' constructions; costs entry
  runs per subclass (time). cases/z451:18.
* **Deleting every key with a prefix from an open record** (not relational: a lens) (`for k in [k for k in environ if
  k.startswith(P)]: del environ[k]`): needs an `∖P*` lens on sources and that loop's pattern;
  costs a new lens kind. stdlib/sm02:148,165,168,169,170.
* **Results in any order** [relational, weakly: the list is a permutation of the input's values] (`sorted()` of `as_completed`'s results): needs a multiset of a
  list's items without positions; costs a list domain. stdlib/sm05:189.
* **Requirements a later method makes on a stored parameter** (not relational; 23: parameter contracts traced through `self` attributes) (object-lifecycle schemas):
  needs requirements on an attribute traced back to the constructor's parameter; costs
  source tracking through attributes. stdlib/s060:29.
* **Either-key requirements** [relational, a contract form: `amount` required iff `amount_cents` absent; 22's `only if` covers a test on a parameter (`if "amount_cents" not in d`), not a pydantic union's validation of a json input] (`amount` or `amount_cents`): needs disjunctive schemas; costs
  a schema form. pydantic/ym04:163.
* **Strings known by a literal prefix / suffix** (not relational) (`"{name} ({age})".format_map(d)` ends in
  ")"; also `getattr(self, f"run_{mode}")`): needs a str value with literal affixes; costs a
  `KeyVal` kind or a label convention. stdlib/s125:47.
* **Ints past 64 bits** (not relational) (`Key::Int` is i64): needs a big-int key; costs a bignum in every int
  model. torch/s147:17.
* **Forward hooks run as perhaps removed** (not relational; no handle tracking): needs a handle object whose
  `remove()` and escape are tracked, hooks kept in order; costs a model object kind.
  torch/s151:47.
* **Row labels of a read_csv frame are 0..n-1** (not relational: an index-kind lens) (a str label of `apply(axis=1)`'s Series
  fails): needs an index-kind lens (int labels only); costs index tracking per frame.
  cases/g55:50.
* **Inverse keys in schemas** (not relational) (`upper⁻¹('QTY')` has no one input name): the harness can only
  state names; needs the input's header. pandas/p120:36.
* **A slice of an input keeps no positions** (z497): `users[1:][0]["name"]` requires
  `users[*]["name"]`, not `users[1]["name"]`; the slice's own length is not related to the
  input's.
* **A recursion that copies its input joins its depths' paths** (`thaw(o)`: `{k: thaw(v)}` /
  `[thaw(e)]`): reads of `rows[*]['items'][*]['parts']` give `json.loads@L['*']['*']['*']`
  requiring `n` *and* `parts` always (`n` is the next level's). Repro: thaw + three nested
  loops over `json.loads` (a scratch `deep7.py`, see z506).
* **Lengths past 15** are not tracked (a list of 20 appends has an unknown length).
* **Slices by unknown bounds** keep no positions or length relation, but a prefix `xs[:n]`
  (z517) (see z497 above).
* **Weak stores through names holding several objects** (`d = a if c else b; d["k"] = 1`):
  neither object gets `k` definitely, though each path stores into one.
* **Runs cut by the deadline** stop where they are: what the rest of the run would have
  found is missing (the budget normally ends the work first).
* **Lists in list-subclass instances:** `append` adds no elements (item row only open).
* **Paths through what a method returned** are left out of contracts
  (`df.groupby(…)["amount"]` recorded on `param F.df['groupby']`): add a `Call` step.
* **`"@" in v` on a str parameter is a key requirement** (pydantic ym01): `require_by_raise`
  takes `in` for key membership on any unknown.
* **Frame column reads behind a `try` are probes, dict reads are required** (pm03 / pm05 /
  p112 vs `caught_load`): two rules; pick one.

## Models

### torch
* **Facts lost at a loop's join** (p04:95 out / h): `torch.stack` of a list a loop appended to
  raises when empty, but the zero-iteration path is joined in before.
* **p06's `generate`** (L57-70): `ids` joined over the loop's rounds loses its rank (open before
  the loop, 2-D after a round), so the model's output there is `[*]`. The back edge holds
  `ids.dim() == 2` (learned in forward), but the parameter is an object whose symbol gives its
  shape: joined with the round's tensor label it is two parts of one value, and their shapes are
  joined only where the value is used (`t_shape`), under the head's facts (none). A join of
  `[*ids]` with `[ids.shape[0], X]` into `[ids.shape[0], ?, *ids.shape[2:]]` needs the back
  edge's fact at the state join. Slices by an unknown negative start (`ids[:, -L:]`).
* Ranged int tensors (`^lo,hi` labels) lose their range through arithmetic (`j + 1`).
* **Forward hooks run as perhaps removed** (s151:47): no handle tracking; pre-hooks and
  backward hooks never run.
* **Layer attributes rowan does not set** (`nn.Dropout(p).p`, `.inplace`, …) read as
  `likely-missing` on the layer (timm `layers/attention.py:127` `self.attn_drop.p`: 618 records,
  hidden in the report only because the group also holds an object rowan knows nothing of).
* **Gaps:** `state_dict()`, `sum(p.numel() for p in m.parameters())`, `named_parameters()`
  names are unknown; dtypes / devices not modelled; no facts from `<` / `!=` on dims.
* **Hover of `len(w)`** prints `w.shape[0]` for strs that are not parameters annotated `str`.
* **timm memory** (~1.4-1.6 GB): every candidate norm layer is constructed; a model of timm's
  layer registry (`_NORM_MAP[name]`) would cut it.

### pandas
* **Cell values are not tracked** (p091:13 value_counts, p094:17 idxmax, pm10:132 grade, b002).
* **Relations between columns** (pm04:107 note absent -> note_y; pm14:139 min / max / mean
  added together).
* **Schemas through a branch-dependent column list or a merge of open sides** (pm08:177,
  pm10:170, p074:19,20): no path correlation.
* **Index as an itertuples field needs the file** (p100:20 absent={Index}).
* **Records into frames** (fastapi/fm04:124,165): record values do not flow into columns /
  groupby keys; empty frames decide no path.
* `pivot_table(columns=…)` columns unknown; `sort_values` / `drop_duplicates` may raise anything.

### other models
* **Logging handlers / filters** never run (`log.addHandler(H())`): a table of handlers per
  logger run at each logging call would be sound.
* **Library code calling instances' methods** (`inspect.getmembers` properties, pprint
  `__repr__`, heapq / bisect `__lt__`, set `__hash__`) is not followed.
* **IntEnum / IntFlag members with value 0** are taken as true.
* **`getattr(None, "x", default)`** gives the str method value (load_attr's scalar path).
* **`e.__class__.__name__`** of a builtin exception is unknown; except-bound names are not
  deleted after the handler.
* **Model objects with attributes** show as `<Match>`; `Counter({})` prints oddly.
* **External sources carry no module** (`csv.DictReader@L48`): put the module in `Source`.

## Performance and memory

* **Generator restarts for unknown code** (rich: one restart of `Console.render`'s loop is
  ~half of its statements, 497k with it, ~250k without): `NULL_FILE` is changed by skipped calls
  whose code reaches `Console.file` (it returns the global) and `LegacyWindowsTerm(self.file)`,
  and `Table.__rich_console__` reads it after a yield (`getattr(file, "rich_proxied_file", file)`).
  Soundly so under name-based escapes (see Precision gaps: returns, stores into instances); only
  a path-sensitive answer (legacy_windows is False) avoids it. The other causes (Box constants by
  `HEAVY_HEAD` default, class attributes by instance stores, iterated class lists, re patterns,
  NamedTuple Segments) are gone. Remaining smaller restarts: `Text@L10.__rich__`
  (`Table._measure_column`), Style copies (`_render_buffer`, 3x), `_render_segments`.
* **Results of library calls that cannot change** (`re.compile(pattern)` of a pattern not known:
  an unknown result with rows; pip restarts `WorkingSet.add_entry` when unknown code "changes"
  it): an unknown result of such a call could be immutable as a known pattern is.
* **Half-built objects past four raises of one construction** are kept in raise states
  (`drop_unreached`: each look enumerates what the analyzer holds; timm's module constructors
  raise at hundreds of points under a try). A counter of side-table writes would let a look
  skip that enumeration when nothing was written since.

* **sktime** (~40 s, 0.57 GB): havoc ~31M visits, mostly skipped `_FlagManager._set_flags` /
  `_get_flags` (update / read the `*_dynamic` dicts, whose values reach estimators: unknown
  code given them walks it all; a skipped method that only reads an attribute's value, or
  calls `.update` on it, could change less), row joins 21M; `walk_from` is ~21% of the time.
* **click** (~8-12 s): `ctx` (~1300 objects) passed to skipped methods; an argument analysis
  like `receiver_effects` for parameters only passed on would cut it.
* **rich / email:** skipped calls' havoc walks repeated on unchanged data (a per-object
  "havocked by this code, unchanged since" mark); `collect_since` per raise record;
  `record_raise_state` ~11% (every unknown call in a try body records a state).
* **Liveness walks** (`live_among`): a reverse index (who holds an object) would answer
  without a walk; stateless function objects need none; `obj_edges` scans all of
  `state.lasts` per object (a map keyed by object would fix it); the variant probe interns 8
  strings per re-allocation.
* **GC at loop heads** before the fixpoint compares states; `gc_pinned` iterates ~16 growing
  side tables at every return.
* **Copies:** building a row key by key is O(n²); interner lookups allocate on hits;
  `limit()` hashes its name per call; `Registry::lookup`
  formats ~70 strings per unmodelled call; `has_module_models` builds a Registry per import.
* **Where the time goes** (CI `perf`, inclusive; local timers agree): heap-map merges / diffs /
  `same` 5-14%, try raise-state joins 4-10%, havoc 6-38% (sktime, click), GC 2-10%, the rest
  spread over statements; loops 2.0-2.3 rounds on average. Reducing statements re-run per call
  site is the lever, not the maps.
* **Nested unrolled loops multiply** (three nested 16-item loops 1.15 s, four hit the
  deadline): bound the product across nesting levels.
* **idna's tables** (pip `PoolManager.connection_from_host` on demand: 1.9-2.9 s in 15k
  statements): `intranges_contain` / `_decode_range` run ~1000 times over the big literal range
  tuples; a summary of those lookups (a bisect over a known tuple is any of its items) would do.
* **Uncapped computation on known data:** StringIO writes, `"a".ljust(10**12)`, `%` widths,
  regex step limit per start position (`re.search(r"(a*)*b", "a" * 5000)`), sqlite clones
  the Db per statement, `sorted(instances)` runs `__lt__` for every pair, polynomial dims
  uncapped, method keys embed literal receivers, `parse_expression` leaks a tree per call,
  `str.split` / `csv.DictReader` / `range_labels` make one object per element.

## Engine simplification

* **Summary objects take the last recent object's payload** (`summary_objects`, gc.rs: the
  site's object is re-allocated with the recent one's payload at each back-edge, not a join):
  a bound method or `Pair` made per iteration keeps only the last iteration's values. Needs a
  repro; payloads holding values are flow-insensitive state (`Pair`, `Bound`, `Method`).

* **Sites and contexts carry structure in strings** (`M<idx>:`, `#<i>`, `gen:`, `entry:`):
  a `Site` struct and an interned `Ctx`.
* **`Obj.kind` and `Obj.payload` are independent**: one enum with data per variant.
* **`models::Tables` is a flat bag of ~90 fields** of every library (pydantic, fastapi, torch,
  sqlite, csv, enums, singledispatch, …): group them per library so each model owns its state,
  and its GC edges with it (`side_edges` / `gc_pinned` list them by hand).
* **The model API is still wide**: besides `interp/api.rs` and the `Heap`, 94 engine methods
  stay `pub` because models call them (listed in `api.rs`). Candidates: models pushing frames
  (`frame_mut`), allocating raw objects (`alloc_raw`), running tries (`enter_try` /
  `leave_try`); `ExcNames` / `Frame` built by models.
* **Model functions take an analyzer they do not use** (`_an`: `t_int`, `t_bool`, `t_str`,
  `t_is_none`, `t_kw`, `t_cfg`, `t_rank_value`, `infinite_elem`, `member_base`, … 14): drop it.
* **Long functions in the models** (the proxy covers the engine only): 59 model functions are
  over 100 lines (`pyvalues::cases` 1457 — a test table —, `str_method` 432, `m_itertools` 430,
  `pyd_convert` 421, `t_method` 402, `sqlite::parse` 347, `groupby_method` 333).
* **One registry of side tables** (`copy_side_tables`, `obj_edges`, `gc_pinned` list them by
  hand and disagree); model state (sqlite, csv, deques…) in `State` instead of ~170
  flow-insensitive tables.
* **`Obj::iter_elem`** now only marks a model's element record (a frame's row, a csv row) as
  one a loop may view (`loop_view`, besides dicts, instances and unknown objects); a kind of
  its own would say so.
* **Branch facts for a test only one call runs** (`Analyzer::tested`): facts are made only for
  tests the function repeats (or that sit in a loop); a fact that served the same `if` in a
  later call of the function is not made (no golden case or benchmark verdict needed one).
* **Recursion and loops could share a fixpoint driver**; narrowing could evaluate a test once
  into two states (`narrow` re-evaluates non-call subexpressions; `call_memo` covers calls).
* **Two copies of list lengths** (`state.lens` and `Obj.literal_len` / `known_prefix`): see
  the soundness item above.
* **Class attributes** live in the state when a path has one, else in `cls_slots`
  (flow-insensitive); all in the state would remove `cls_slots`.
* **`KeyVal` / `.kv`** name the scalar half of a value by its first use: rename to `Scalar`.
* **Kinds told apart by label prefixes** (`"defaultdict@"`, `"list@"`), stdobj kinds by payload
  strings with magic keys; generators, ranges and maps are `Kind::List` (isinstance list?).
* **Leftovers:** `Ns` packing in the trie key has no assert; the
  library knowledge still in `load_instance_attr` (pydantic privates, Thread, NamedTuple).

## Editor and extension

* **On-demand tails left** (`bench/hovers.py`; `rowan hovers FILE --only NAME` times one):
  sktime `BaseObject.create_test_instances_and_names` / `create_test_instance` (17.6 / 10 s:
  `cls(**params)` on ~1089 subclasses is one construction each, each a skipped `__init__`
  with its own closure and walks: constructions past the cap could be one, the instances
  allocated per class); `BaseObject.clone` / `reset` (5-7 s: making the 1089 any-instances,
  2.9 s, and their runs); timm `TokenDistillationTeacher.forward` / `_resolve_teacher`
  (2.7-3.4 s: `create_model(name)` with an unknown name, every model's skipped calls walking
  the model graph). `code_closure` is recomputed per root set (~1 ms each on sktime): an
  incremental closure (rounds of `follow_code` extend the last one) or a per-function piece
  validated by the values it read would cut it.
* **An entry run whose loop rounds explode in objects** (timm `models/visformer.py` as the
  entry: `_create_visformer(**kwargs)` -> `adapt_model_from_string`'s
  `for n, m in parent_module.named_modules()` makes 6k, 110k, 830k, 1.2M objects in
  successive rounds; without a deadline the run took 621 s / 7 GB, 4.5M verdict records;
  with it, ~13 s and 3-4 GB). Something in the round (deepcopy / set_layer of a model, or
  placeholders per receiver) allocates per existing object, not per site. In `rowan serve`
  this is the whole run beside the module code's answer: the server's peak (2-3.3 GB on
  timm model files) is this run's.
* Setups that cost: timm's package import ~3.5 s (the first hover in any timm file).
* Contracts: the per-call mark is the worst read of what the call passed (verdicts of that
  call's run reaching the passed objects, its callees too); tensors only by shape errors (a
  requirement decided true leaves no trace); `self`'s attributes a method reads are not a
  contract yet (reads of known instances record no requirement); a parameter's contract is
  shown at its `def` only, not at its uses.
* **General values of long functions are long** (apps/small/a002 L96 `settings`: a 7-line
  `Settings(…)` with every field's sources): the general value could be cut harder than the
  calls' one, or lead with the fields the calls do not decide.
* **`__init__`'s own lines show two general states** (z532 L35 `self`: `Counter() |
  Counter(n=0 | start)`): its general-instance run and its requirement-only run on an
  instance already built; the latter could skip recording facts.
* **Closures and lambdas get no general run** (the quiet-run filter in `analyse_uncalled_range`):
  their hover's value is still the join of the calls; a general run would need their cells
  (`closure_states`, as torch modules do).
* **Torch hovers for any call** (whole program and on demand): library layers called and `self(...)`
  change nothing now (nanoGPT, p02, m05 alike); left: a method handing `self` to a library
  function (`self.apply(fn)`), p05 / s127 (attributes stored by methods: `last=None | ?`).
* **The whole run in `rowan serve` cannot be stopped** when its file changes (`changed`): its
  answer is dropped, but it runs to its deadline. A cancel flag checked with the deadline
  (`Budget::tick`) would end it at once; `EARLY` (1 s) is a constant, not a setting.
* **The module code's own calls past a work cap**: a file whose module code (outside
  `__main__`) does much (rich `color.py`'s tables) still sets up slowly; skipping its calls
  past the hover cap would bound the setup, at the cost of its globals' values.
* **A timed-out request leaves the analysis running:** when `rowan.timeoutSeconds` (60 s)
  fires, `extension.js` `serverRequest` only stops waiting; `rowan serve` keeps running and
  holding memory. Kill and restart the server on timeout; consider a memory cap too.
* **A dataclass field's default `field(default_factory=list)`** hovers as "not known
  statically" (apps/small/a002 L23): say what each instance gets.
* **Alternatives cut alike** show once now; better: put the fields that differ first
  (a002 L57 `Settings(…retries=2)` / `(…retries=5)` differ past the cut).
* **A hover of `self` in `__init__` shows two states** (`OrderRepo() | OrderRepo(_seq=0, …)`).
* **Hovers don't name an open dict's computed keys' origin** (`store(…)` in the facts).
* **Values joined with the shared unknown object** print `1 | somelib.f(d)`: no hover test
  covers unknown calls.

## Test harness

* **Oracle flake:** a full run with `-j 3` now and then reports one case fewer (918/919, and
  993/994 in iteration 17; four reruns agreed): find the case (print the disagreeing one). In
  iteration 18 it showed as a `timeout` of `y488_sometimes_twice.py` (0.5 s alone).
* **`ROWAN_DEBUG=1` slows a run by ~25%** (stdlib12k 2.5 -> 3.2 s: a line per body run), and
  the bench times its runs with it: time a run without it, take `work:` from a second run.
* **The bench's `✗/⚠` count** counts report lines; count `--json` verdict records instead.
* **`#@todo`s that are prose** must keep their reason in parentheses after the expectation
  (`ROWAN_TODO=check` strips it); a directive-only to-do applies to the code line above it.

## Ruled out (tried; why not)

* A bound on the product of the unrolled loops' item counts across calls (for GPT): a loop
  over distinct receivers run as a fixpoint costs rounds × receivers (each receiver is an
  alternative), more than unrolling; only one summary receiver makes it cheap.

* A method on several receivers as one call per function in whole runs too (as on demand):
  click 298 -> 257 sites (entry runs lost), functional 36 sites weaker; kept to calls past the
  budget and runs on demand.
* `code_closure` scanning each class once and then only by the names new since (instead of
  every class's attributes each round): slower (the per-round set differences cost more).
* Symbolic summaries (`parked/joined-summaries`): unsound and costly.
* A separate budget for entry runs: click 186 -> 209 sites but 12 -> 21 s, rich 2.9 -> 6 s.
* A fresh budget per entry run (for order independence): stdlib12k 7x the work, still
  order-dependent (flow-insensitive tables). A per-run share (B/4) of each code's budget: little
  change either way.
* The `__init__` run that makes an entry run's instance as requirement-only (no verdicts):
  pip 1343 -> 1000 sites, sktime 550 -> 211; those runs carry much of the libraries' coverage.
* One unknown call for several unknown callees: havoc +22%, schemas changed.
* Receiver effects for every parameter (upper bound): 4-9 fewer `likely`, no `budget=0` gain.
* An unbounded havoc memo: cost more than it saved (a bounded one is in).
* Recursion table kept across activations: sktime 69 -> 122 s.
* `slotmap-trie` branch (slots as a trie of tries): same answers, no gain, more memory.
* Hash-consing the heap maps: merges / diffs / `same` are 5-14% of the time, ~55-65% of merge
  visits are below equal but unshared subtrees (half in try raise-state joins), trie nodes peak
  at 13-105 MB (<= 28% of RSS): a 3-9% ceiling before the cost of the table.
* Raise states of a try joined incrementally (only what differs from the last one): slower
  (stdlib12k joins 172 -> 253 ms): per-key reads and writes cost more than whole merges.
* Re-sharing the maps with the pre-call state after GC at returns: no change.
* `IdSet` with an inline single id (enum): 1-3% slower, 5-10% more memory (24-byte sets).
* Earlier widening at loop heads (whole head from round 3 / 5 / 8, or only the list-length
  alternatives `join_len` mask from round 2 / 4, the cause of loops of 8-16 rounds): golden
  passes, more proven (email 537 -> 565, pip 1330 -> 1379), but email 2.5 -> 3.1-3.4 s. At 12,
  email +12%; from 16, no change.
* Worklist / WTO solvers with chosen widening points: no loop reaches the 40-round cap.
* BDD / union-find sets: value and id-set operations are <= 3% of the profiles.
* Sparse or demand-driven propagation: copies are O(1) and joins cost the differences already.
* A larger budget for email: proven 233 -> 332 at 4x budget, `likely` barely changes.
* A call memo by entry state (branch `parked/memo-probe`, a probe): a body run whose whole entry
  (state, arguments, context, callers' names, GC roots) equals the previous run's at that call is
  5-14% of statements (timm 1%), but nearly every one sits in a loop round right after code that
  changed heap objects outside `State` (recent / summary payloads, merges at back-edges):
  sound hits (nothing outside `State` changed since the earlier run) are 0.3-2.4% of statements,
  less than the probe's own bookkeeping cost (+20-35% time); all of them replayed exactly. Reads outside
  `State` (payloads, `m` tables, class slots) would have to be tracked to do better.
