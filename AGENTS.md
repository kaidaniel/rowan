# AGENTS.md — rowan, handoff notes

Static key-set inference for Python records: infers, for every dict-like value (dicts,
DataFrame columns, object attributes, kwargs, JSON/CSV rows, argparse namespaces…), which
keys it has at each point and which keys the program demands of it; demands on external
inputs become inferred schemas. Design: `SPECIFICATION.md` (17 sections, Appendix A golden
cases). User-facing summary: `README.md`. This file is for whoever picks the work up.

## State of the code

One engine, in Rust (`rowan/`, ~14k lines). It began as a port of a Python prototype
(frozen as the oracle while porting, differential-tested to identical verdict sets, then
removed in the commit after `a1f4d7a`); since then it has grown algorithmic improvements and
semantic fixes that the prototype did not have, so the prototype is history, not a reference.

| file | role |
| --- | --- |
| `rowan/src/intern.rs` | thread-local interners: keys (`Key` → `KeyId`), strings, sources, lenses; `IdSet<T>` = sorted `Arc<[T]>` with merge-based set ops (`KeySet`, `ObjSet`, …). |
| `rowan/src/domain.rs` | lens steps, `Source`, `Row` (`Arc` + cached hash, all sets as `IdSet`s), `classify`, `KeyVal`, `ReqInfo`, `Value`, `chain_extend`. |
| `rowan/src/layered.rs` | `LayerMap` / `TwoLevel`: frozen layers with a private top, O(1) copy, O(diff) join/same/diff_keys, `copy_ref` (copy from a shared reference, used for `exits`); randomised tests against a naive model. |
| `rowan/src/state.rs` | `Kind`, `State` (env, narrowed, rows, slots, elems, reqs, ret, exits as `Arc<State>`, symbolic), `Footprint`. |
| `rowan/src/scopes.rs` | lexical scopes (locals / cells / free / globals) over the ruff AST. |
| `rowan/src/frontend.rs` | parsing (`ruff_python_parser`), leaked `&'static` module ASTs, a thread-local registry mapping any node address to its module so a node carried across modules (a deferred access in a summary) still knows its file and line; `Node` = located node (synthetic nodes for internal fake calls). |
| `rowan/src/interp/mod.rs` | `Analyzer` struct, `Options`, `Obj`/`Payload`, `FuncInfo`/`ClassInfo` (`Rc`, `Cell` for the mutable flags), allocation (`alloc`, `alloc_raw`, `alloc_in_frame`), verdicts, entry points, `rename_recent`, `record_fact`. |
| `rowan/src/interp/{stmts,exprs,funcs,narrow,access,calls}.rs` | statements, expressions and comprehensions, functions / names / targets / iteration, narrowing, record access and attributes, calls and bindings. |
| `rowan/src/summary.rs` | placeholders, `SymReq`/events, `summarise` (rounds, partial and provisional summaries), `Inst` (instantiation with its caches), `focus_groups` (allocation-site resolution). |
| `rowan/src/models/{mod,pandas}.rs` | library models; `Registry::lookup` with prefix / allowlist fallbacks. |
| `rowan/src/report.rs` | dedupe, schema table, text, JSON lines, `--golden` and `--facts` documents. |
| `rowan/src/main.rs` | CLI; runs the analysis on a thread with a 1 GiB stack. |
| `rowan/py/` | PyO3 binding (`rowan.analyse(source, …)` / `analyse_file(path)` → `--golden` JSON string). Builds with maturin; see known problems. |
| `tests/run_golden.py`, `tests/cases/` | golden harness, 103 cases: `a01`–`e36` from the original design (Appendix A and the prototype era), `f37` (recency), `g40`–`g65` (AST node coverage, closures taking closures returning lambdas, reassignment, control flow, inputs indexing results of input functions, classes, string keys, containers, guards, modules, generators, recursion, kwargs forwarding, configs, pandas, annotations, mutation through layers, comprehensions, dispatch, a CLI script, dict methods, a larger mixed program), `h66`–`h72` (larger programs passing objects around: an event bus with closures / bound methods / partials in per-topic lists, a fluent builder and a stage pipeline, a plugin registry with a class decorator and a recursive visitor, a dependency-injection container resolving services in constructors, an order state machine with a nested transition table, a dataclass graph with a stack walk and a defaultdict grouping, a middleware chain of closures over closures), `i73`–`i92` (layered config, an inventory of objects, properties with setters and observers, a tree built from a flat list, command objects with undo, a record pipeline with map / filter / sorted / setdefault, a memoising decorator, context managers, generator chains, exception paths, an inheritance chain with a mixin, NamedTuple / TypedDict, computed string keys, kwargs forwarding, a task queue, a JSON round trip, ranking with enumerate / zip, nested path helpers, feature flags, accumulators), `j93` (one 800-line order-management program: layered settings, dataclass entities, repositories keyed by functions, an event bus, a rule-chain pricer, a guarded state machine, a middleware router, reporting with plugins, a recursive org tree, three scenario runs; all 86 checks pass), `k94_pkg` (three flat modules importing each other), `l95_pkg` (a real package with `__init__.py` re-exports, a subpackage and relative imports), `m96_pkg` (a 1,700-line warehouse system as a package of twelve modules: layered config with profiles, dataclass entities with a dataclass subclass and `__post_init__`, an inventory with FIFO lot consumption and function-keyed indexes, receipts validated by a rule chain, an order book with a repository, pick planning with decorator-registered strategies and box packing, shipping with rate rules and a guarded state machine, an event bus, reports with plugins, a scheduler with generators and a context manager, labels, cycle counts; three scenarios, 155 checks), `n97_pkg` (installed packages: a followed one analysed from a fake site-packages directory, a modelled one, an unfollowed one), `o98`–`o103` (numeric flows deciding keys; positions through filtered comprehensions, sorted with tuple keys, enumerate / zip / slices; a decorator registry filled by a package's submodules; a three-level dataclass chain with `__post_init__`, a classmethod constructor, `yield from` and nested context managers; keys built from strings; exception paths around record access), `p104`–`p105` (stores through unknown list indexes; an entry point storing into open content). The `h`, `i` and `j` cases were written as value-analysis specifications *before* running the engine; the engine was fixed until it produced them, and the few expectations that turned out wrong on re-reading carry a comment saying why. `#@todo` directives mark expectations the engine does not meet yet: the harness counts them and does not check them unless `ROWAN_TODO=check` is set (none at present). | Directives: `#@ tier key [; …]`, `#@row`, `#@schema`, `#@todo`, `#@argv` (extra engine flags for the case; relative paths are resolved against the case's directory); a `#@` directive must be the last thing on its line. |
| `tests/run_snapshots.py`, `tests/snapshots/`, `tests/fixtures/` | `--json` output snapshots on `examples/etl_report.py` and three large fixtures (modules of the old prototype kept as analysis *inputs* only). The regression guard for refactors of the internals; `--update` after reviewing a diff. |
| `vscode-rowan/` | VS Code extension (plain JS): talks to `rowan serve` (one process per binary; falls back to `rowan FILE --facts`) on open/save, hover = facts for (line, name), verdicts as diagnostics; `node test.js` checks the rendering. |

Run (Python only for the harness, via `uv`; `.python-version` pins 3.14):

```
cd rowan && cargo build --release && cargo test --release --lib
rowan/target/release/rowan FILE.py [--json|--golden|--facts] [--no-rows --no-summaries --no-entry --no-imports]
uv run --no-project python tests/run_golden.py      # 63/63
uv run --no-project python tests/run_snapshots.py   # SAME on all snapshots
ROWAN_SUMMARIES=0 …                                 # old per-call-site engine (34/63; kept for comparison)
```

Env: `ROWAN_DEBUG=1` (entry-point progress, slow instantiations, which event makes a call
site tabulate and which call sites fall back to the summary), `ROWAN_NO=…` with the option
names `recency,scc,site,narrowed,positional` and the bisecting switches
`paramnarrow,partialclear,usermodel,dispatch1,inph,isinst,ddfactory,ddnarrow,boundtmpl,bindsub,ifexp,dcfield,split,builtincls,clonectx`
(`Analyzer::disabled`; `sigsum` is the summary specialisation by argument classes, on by
default), `ROWAN_ON=…` (experimental refinements, `Analyzer::enabled`; none at present),
`ROWAN_LIMITS=ms=2000,tab=256,call=256,clones=256,chain=2,path=3` (the budgets: the time
after which the analysis stops refining — no new tabulations, allocation sites collapse onto
their context-free object — then the hard caps: literal-event and call tabulations per
function, clones per allocation site, readable call-context depth before hashing,
placeholder path depth; `Analyzer::limit`). `ROWAN_DEBUG=1` also prints the effort spent
(`work: … tabulations, objects, ms`); `ROWAN_DEBUG=2` traces every event recorded.

Row notation in reports and hovers: `{"a": 1, "b"?: {...}, "c"~: ..., **}` — definite
keys plain, `?` set on some paths only, `~` unless unknown code removed it, `**` open, values
rendered as far as known (`report::render_value`: literals, `1 | 2`, positional lists,
`Cls(a=1, **)`, `...` unknown; depth 3, 8 items, cycles stop); `Row::compact` is the keys-only
form, `Row::details` the internal form with sources / mutators / terms (printed by the
harness on row-check failures). Facts record the rendered text at recording time
(`Fact::texts`; `--facts` `text`).

## Design notes

* Ids everywhere: keys, sources, lenses and site strings are interned; rows hold sorted id
  vectors, so join/compare are merges; `Row` is an `Arc` with a cached hash. Object ids are
  indices into `Analyzer.objects` (index 0 is a dummy so ids are 1-based in labels like
  `dict@L3#5`).
* The layered maps keep the fork/join structure the design needs (O(diff) join and
  footprints via common-ancestor layers). One subtlety: an inner map seen on both sides of
  a join is only "inherited" if the ancestor layer holds that same `Arc` (`copy_ref` can put
  one map in two layers above the ancestor) — see `join_with`.
* AST nodes are `&'static` (parsed modules are leaked for the process; a library user
  analysing many programs in one process should expect that). The registry stores both the
  `Stmt`/`Expr` enum address and the payload struct address — the interpreter uses either.
* Where the old prototype iterated a `set`, the engine uses sorted-by-repr or smallest-id
  order; everything is deterministic.
* `exits` is an `Arc<State>`; joining it with itself is a no-op.
* Call contexts are keyed by call site line *and* column; instance allocation sites include
  the class; decorators are applied at the decorator's own site; clones of distinct
  templates that land on one object (levels of a recursive structure) combine their facts.
* A summary's entry snapshot is taken before parameters are bound, so cells holding captured
  parameters and the `*args`/`**kwargs` containers are instantiated per call; a data cell
  read inside a nested summary is an entry fact (path placeholder) of that summary.
* Requirements recorded under an `if`/`try`/boolean operator inside a summarised function
  are "sometimes" (`branch_depth`); inside a loop body they stay per-element (`always`).
* Flow: `if` prunes a branch when the test is definitely truthy/falsy (truthiness of a set
  of literals when they agree; `x is None` / `x is not None` when `x` is a known literal or
  only holds objects; `==` / `!=` between two single literals); `k in d` on a closed
  record without `k` makes the branch infeasible (so `__contains__` guards and
  `assert "k" in d` can make code dead); a guarded load in a real `try` narrows the key in on
  the fall-through path; a successful access narrows the key in.
* Object identity across activations: a function tabulated inside another tabulated frame
  gets a context chained to the frame's (`L10:4/L7:11`; beyond `chain` levels the prefix is
  hashed, `h3b9259db/L10:4`, so activations stay distinct at any depth); methods get
  `o<receiver>/<site>`; calls and allocations inside unrolled loop
  iterations carry the iteration index (`#i`), so each iteration is straight-line code. When
  an allocation site is nevertheless hit again while the earlier object is still reachable
  (`Analyzer::is_live`: from every active frame's locals, overlays, cells, module objects),
  the object becomes weak instead of being re-initialised. An unreachable earlier object is
  overwritten as before (recursion through the same site).
* Tabulation triggers: besides concrete bindings, an event on a path placeholder whose key
  is a parameter-slot key (`d[k]` with `k` a parameter) is decidable when the parent is
  concrete even if the slot is absent (truth / narrow / iter); `binding_of` resolves such
  keys through the parameter's binding; `dict.get` with a default, `setdefault`, a
  parameter-keyed load from a closed literal dict, and undecidable `is None` / `==`
  comparisons record events; a callee summary instantiated inside another summary hands its
  events up to the enclosing summary (`propagate_events`), so the outer call site tabulates.
* Placeholders as lists: `append`/`extend`/… on a class-less placeholder are list models; an
  append through a placeholder is recorded as the i-th append and becomes a positional
  append on the actual list at instantiation (`append_slot_key`); element slots are always
  joined at instantiation. Unpacking a placeholder iterates it (an `iter` event); an entry
  tuple's positions inside a summary are deferred loads. Weak (loop) literal containers keep
  per-position slots, joined across allocations. An empty literal container iterates zero
  times.
* Models added for the `h` cases: class decorators (applied like function decorators),
  `reversed` keeps positions, `dataclasses.field(default_factory=…)` calls the factory per
  construction site, `defaultdict` loads insert the factory value under literal keys (the
  row stays closed; unknown keys open it), `dict.get` on a key absent from every closed
  target is the default alone, unknown-call results have open attribute rows.
* Exceptions: `raise` outside a handler that catches it marks the running call as raising
  (`raise_stack`; summaries carry `raises`, cache hits too); a user call marks the enclosing
  `try` body as "may raise" only when its callee reported it; a call whose every path raises
  leaves the caller unreachable. So `except` branches are dead when the callee provably does
  not raise for these arguments (golden `i82`).
* Values decided from literals: `k in d` on concrete closed records, `not x`, `==` between
  single literals; `dict.pop(k, default)` returns the default alone on a closed record
  without `k` and the value alone when `k` is definite; `list.pop()` / `pop(i)` on a
  positional list outside loops is positional; `max` / `min` of a container are its element;
  `map(f, xs)` over known items keeps positions (one call per item); `sorted(xs)` keeps the
  count (every position may hold any item); `enumerate(xs)` over known items yields literal
  indices; `xs[a:b]` with constant bounds on a positional container keeps positions;
  `"%s_%d" % (a, b)` and `"lit" + param` rename parameter keys and record an event so the
  call site decides them.
* Property setters (`@x.setter`) run on attribute stores, on instances and on typed
  placeholders; function objects hold attributes; `with` calls `__exit__`; generators yield
  into one list per activation, positional for straight-line yields.
* Copies: `dict(x)` / `copy` of an entry object inside a summary remember the original
  (`Obj::copied_from`): deferred loads go to it while the copy's row is purely symbolic, events
  on the copy reach the original's binding, and a copy of a placeholder does not count as a
  concrete binding (so the enclosing call site tabulates instead). The `**kwargs` dict built
  from a forwarded placeholder is such a copy.
* Loops: a call-cache replay inside a loop registers the recent objects it re-creates, so
  the back-edge folds them; the loop-head join keeps requirements recorded in the body as
  "always" (`State::join_loop`).
* Method lookup on a class-less placeholder: a builtin class declared for it
  (`isinstance(x, dict)`, `self.xs = []`) selects the model family only; `dict`/`list`
  method names use the models, then scalar / str names (minus `add`) are unknown, then
  user classes defining the name are candidates (a dispatch event; when both a model and
  user classes apply the value is their join with a narrow event, so the call site decides).
  Fields assigned from constructor parameters take the class of the instances passed at
  construction sites (`ctor_field_classes`), which keeps `self.pricer.quote` on `Pricer`.
  Resolving user methods before the scalar fallback cost 40× on the state fixture (user
  `copy` / `remove` through every placeholder) and is not done.
* Also from the `j93` work: `isinstance` decided on concrete values, `dict(x)` of a literal
  string `split` positional, dataclass field defaults are instance data, clones of templates
  keep the template's own context in their site (two objects built by one constructor in one
  body stay distinct), bound methods / method values made in a body are templates (cloned
  with their receivers), a summary that did not converge is final after its rounds, `IfExp`
  prunes on a decided test, bindings substitute a summary's own placeholders held in cells,
  parameter-keyed loads through placeholders and `in` on placeholder records record events,
  `defaultdict` calls its factory per inserted key (`dd_factories`, copied to clones).
* Summaries are keyed by (function, code cells, typed cells, argument-class signature); every
  key variant gets its own allocation context (`sum<f>`, `sum<f>v<n>`), so the placeholders
  and templates of two variants never collide (they did: the second summarisation
  overwrote the first's placeholder classes and rows). A class that comes from the
  signature is exact (no dispatch event for subclass overrides); a typed placeholder
  receiver contributes its class to nested signatures. `bool(x)` is the literal when the
  truthiness of `x` is decided. The generated `__init__` of a dataclass base runs for
  subclasses, through `super().__init__` and `Base.__init__(self, …)`. Fields assigned
  builtin containers (`self.x = {}` / `[]` / `defaultdict(list)`) carry that builtin class in
  summaries (`infer_field_classes`), so e.g. defaultdict insertion is decided per call site.
* From the `m96` package (what a 1,700-line multi-module program needed): allocation and
  call sites carry the module (`M3:L12:4`; the main module's stay `L12:4`), since two
  modules allocating at the same line and column collided into one object; comparisons
  between known numbers or strings (`<`, `<=`, `>`, `>=`) and arithmetic on known numbers
  (`* // / %` as well as `+ -`) are evaluated, with a truth / narrow event when a side is a
  parameter so the call site decides; `len()` of a known-length container, a literal string
  or a closed record with only definite keys, `sum()` of known numbers (also over a
  generator expression), `bool()`, `"%03d|%.2f" %` with width and precision, and
  `str.partition` are evaluated; a decided `if` test leaves straight-line code (appends stay
  positional, requirements stay "always"); list comprehensions keep positions when their
  items are known and every filter is decided; `sorted(xs, key=…)` keeps the order when every
  key is a known literal or a tuple of them (stable; `reverse=`); appends of unknown count
  keep the positions below them valid (`Obj::known_prefix`); `extend` with known items is
  positional; the generated dataclass `__init__` calls `__post_init__`; `with` calls
  `__exit__(None, None, None)`; an empty `Counter()` is a closed dict whose missing keys read
  as 0; a membership test against a positional container of known literals is decided; a
  dict / list method name on a class-less placeholder uses the container model alone (plus a
  dispatch event) instead of also running every user class defining that name, which had
  spread `Config.get` over every `row.get` and made it recurse; `if x.get("k"):` no longer
  narrows a user instance (its `get` is its own method; narrowing its closed item row made
  the branch infeasible).
* From the `o` cases: a classmethod reached through a class binds that class (`Sub.of()`
  constructs `Sub`, not the class defining `of`); a `try` body that ends with `return` /
  `break` / `continue` did not raise (the handlers are dead unless something in the body may
  raise); a guarded access that is definitely missing raises there (the rest of the try body
  is unreachable, so the handler runs alone, and a `return` whose value's evaluation raised
  records no exit); `%`-formatting and `str.join` with a parameter among the arguments record
  an event so the call site decides; `str.join` of known strings is evaluated.
* Open content: the object standing for the content under an unknown key (`x[i]` with `i`
  unknown, `Kind::Unknown` path objects with key `*` or a parameter slot) stands for many
  values, so stores on it are weak, and a weak store on open content joins with the unknown
  prior value (this was the fixture's false ✗: a strong `payload = PH(...)` through
  `self.objects[oid]` on an open `self` wiped the element's unknown content, and a later
  `self.objects[f].payload.qualname` saw only the PH). At instantiation, a deferred load or
  store `xs[i]` with `i` bound to a non-literal reaches every position and the element slot
  of a list (it was dropped before). A `while` test decided at the loop head prunes: false
  means no iteration, true means no fall-through (the loop leaves by `break`).
* Budgets: `MAX_MS = 2000` is the soft limit — past it the analysis degrades (summaries
  instead of tabulations, no new clones) and the output is still deterministic for a given
  machine speed, only less precise on inputs that take longer; the hard caps are
  `MAX_ACTIVE = 4` activations of one function on the tabulation stack,
  `MAX_TABULATIONS = 256` literal-event tabulations per function, `MAX_CALL_TABULATIONS = 256`,
  `MAX_CLONES = 256` per allocation site, `MAX_CHAINS_PER_FINDING = 8` call chains reported
  per finding (the shortest). The old small caps (24 / 32 / 8) were what kept the three
  `j93` scenarios imprecise: every second scenario's objects collapsed onto the first's.

## Options (default on; `ROWAN_NO=…` switches one off)

* `site_resolution`: at instantiation a placeholder's targets are grouped by (kind,
  allocation site, entry rows); one exact concretisation per group (`Inst::focus_groups`).
* `recency`: an allocation inside a loop (frame `single_nl`, i.e. single ignoring loops)
  returns a strong *recent* object (`site#recent`); at the loop back-edge `rename_recent`
  folds its rows/slots/elems into the weak per-site summary object and re-points every
  reference written during the iteration (footprint vs the loop head, env, narrowed, ret,
  overlays, payloads of objects allocated in the iteration, class attributes). Module-level
  name bindings inside loops are strong under this option. Break and return paths keep the
  recent object. Golden case `f37`.
* `scc`: a summary computed while it used the *partial* summary of a function further down
  the summary stack is provisional (`summary_provisional`, with its dependencies); dropped
  when that function starts a new round or finalises, so callees are re-summarised against
  each round. Sounder and noisier than using the stale partial summary; ~35 % slower on
  large inputs.
* `narrowed_same`: `State::same` compares `narrowed`.
* `positional`: unpacking a literal tuple key yields per-position key values.

## Measured (release build, Apple M-series; "✗" = definitely-missing lines in the report)

| input | time | notes |
| --- | --- | --- |
| golden suite (92 cases) | 0.5 s | one process per case |
| `tests/cases/j93_order_system.py` (830 lines, deep call graph) | 0.09 s | all 86 checks pass |
| `tests/cases/m96_pkg/` (1,716 lines, twelve modules) | 0.10 s | all 155 checks pass |
| `tests/fixtures/proto_summary.py` (1.2k lines) | 2.5 s (hits the 2 s soft limit) | 0 ✗, 11 ⚠ |
| old prototype `models.py` (1.7k lines, reaches a whole interpreter via a typed field) | 41 s | 165 ✗ (false positives, see below) |
| old prototype `interp.py` (3.4k lines) | 69 s | 961 ✗ |
| 137 scripts from local projects | 0.8 s total | no failures; precision not assessed |

## Known problems

* **PyO3 wheel does not load on this machine.** `maturin build --release` in `rowan/py`
  produces a wheel, but importing it fails with dyld "mis-aligned LINKEDIT string pool" on
  macOS 26 / Xcode 26 (Apple clang 21) with rustc 1.91, also with `-ld_classic` and after
  `strip`. The CLI binary is unaffected; the harness and the extension shell out to it. Try
  a newer rustc or Linux before touching the binding code.
* Large self-referential programs (the old `models.py`/`interp.py`) are fast now but noisy:
  the ✗ there come from two families — stores through a sibling closure's cell seen only via
  the factory's summary (`register`-style tables), and dispatch over values of a `Dict[int,
  Obj]` whose contents the entry-point pass does not know. Real-repository validation only
  measured robustness (no panics, no timeouts) and tier counts, not precision — the
  spec §15.4 runtime oracle is still to do.
* Recency: flow-insensitive side tables other than `cls_slots` (`entry_instances`,
  `rec_table` states, `call_cache` entries) are not renamed; the cache keys include the heap
  so stale entries do not match, they just waste memory.
* Mutual recursion still iterates at most `MAX_SUMMARY_ROUNDS = 3` rounds with
  join-widening; with `scc` on, each round re-summarises the dependent callees.
* `Row::concretise` pre-resolves the referenced entry rows transitively before substituting;
  a row whose entry rows reference many placeholders resolves all of them even when a lens
  kills everything.
* Comparisons involving an unknown value keep both branches (known numbers and strings
  are compared); class
  attributes are not records; `match` patterns do not narrow (spec §16); no `getattr` with
  computed names; pandas ops with data-derived columns stay open.
* A store through a pointer to several objects is weak on each of them, even when every
  path stores (`resp["logged"] = True` after `resp = a() if c else b()`): the joined row
  shows the key as `?` (golden `h72`, `maybe`). Path-sensitive ownership would fix it.
* The result of an unknown call whose arguments hold records may alias those records (its
  item row is their join with a mutator); its attribute row is open.
* The call-chain hashing keeps activations distinct at any depth, so recursion that is not
  stopped by `MAX_ACTIVE` allocates per level; the soft time limit is what bounds it.
* Leaked ASTs (see above); a long-running server should re-exec or accept the leak.

## Next steps (agreed plan, in this order; one commit per step)

1. **Values in the row notation and the hover.** Render known values, not just keys:
   `{"a": 1, "b": {"c": [1, 2, 3]}, **}`; a value with several literals as `1 | 2`; objects
   whose contents are unknown by kind (`dict`, `list`, `Cls(...)`); `...` when nothing is
   known; depth and width limits with a cycle guard. Non-record names get their value too
   (`x: 1`). Done in the engine (`Analyzer::render_value` at fact-recording time, `--facts`
   gains `text`, the text report's module-level records use it) and in the extension.
2. **Cross-module analysis, verified and documented.** A multi-file golden package
   (`k94_pkg`, flat siblings) checks that summaries, classes and module dicts flow.
3. **Finish `sigsum`** (summary specialisation by the classes of the receiver and of
   instance-bound parameters, `ROWAN_ON=sigsum`): it closes the `j93`/`k94` dispatch gap and
   is also what makes a cached library summary reusable across callers (a summary that has to
   decide something about its arguments is tabulated at the call site today, charged to the
   caller; a signature-keyed summary is the cacheable unit). Regressions to fix first:
   `h67`, `h68`, `i77`, `i83` (a self placeholder typed with the *subclass* changes
   `super()` chains and the symreq class guards).
4. **Real packages.** `package/__init__.py`, `import a.b.c` (the chain is imported and each
   submodule becomes an attribute of its package), relative imports (`from . import x`,
   `from ..a import b`, against `ModuleInfo::pkg`), `from pkg import sub` falling back to a
   submodule, `--search-path DIR` (repeatable), and the directory above the analysed file's
   outermost package as an import root (`package_root_of_file`). Golden `l95_pkg`
   (`l95_real.py` next to a `shop` package with a `util` subpackage). Site-packages are
   still not searched.
5. **A ~2,000-line example as a package of several modules**, specified by hand first (as
   `h`/`i`/`j`), exercising steps 3–4; fix what it uncovers. Done as `m96_pkg` (1,716 lines
   with the entry); the fixes it needed are listed under "From the `m96` package" above.
   Five earlier expectations that said "the comparison is not evaluated" now follow the
   program (`g47`, `g51`, `g56`, `i74`, `i75`, `i78`, `d31`, `j93`'s free shipping).
6. **Incremental analysis.** Done as far as it pays today: `rowan serve` (JSON lines over
   stdio: `analyse` / `facts {path}`, `changed [paths]`, `shutdown`; `main.rs::serve_loop`)
   keeps, per analysed file, its result and the content hashes of every module the analysis
   loaded, and answers from the cache while none changed; parsed trees are cached per
   (path, content hash) (`frontend::PARSE_CACHE`), so a long-lived process parses each
   version of a file once and the leak stays bounded by versions. The extension spawns one
   server per binary (`rowan.useServer`, default on), sends `changed` for the saved file
   and `analyse`, and falls back to the CLI if the server dies. Measured on `m96_pkg`
   (1,716 lines): first analysis 160 ms, unchanged repeat 8 ms round trip, after editing an
   imported module 125 ms. What is *not* done: reusing the summaries of unchanged modules
   inside a re-analysis. They reference process-local object ids, placeholders and template
   objects; reuse would mean keeping one `Analyzer` alive across runs, resetting its per-run
   state (verdicts, facts, states, call cache, budgets) and dropping the summaries whose
   function or transitive callees live in a changed module. The engine's time is dominated
   by tabulations driven from the entry module, which a change there redoes anyway, so the
   gain is bounded; worth doing once a real project shows entries that take seconds.
7. **Site-packages, allowlisted.** Done: `--follow PKG` (repeatable / comma-separated, or
   `ROWAN_FOLLOW`) makes `import PKG` look in site-packages after the local roots:
   `--site-packages DIR`, `$VIRTUAL_ENV/lib/python*/site-packages`, and a `.venv` / `venv`
   under the file's directory or an ancestor (`main.rs::site_packages_for`; no interpreter
   is run). A package with a registered model (`models::has_module_models`) is never
   followed. Golden `n97_pkg` (`#@argv` gives the harness per-case flags): a fake
   site-packages directory with a followed package, a fake `pandas` that the model must win
   over, and an installed package that is not followed. The server takes the same flags
   (`rowan serve --follow …`), and the result cache keys include the followed modules'
   hashes. Not pandas / numpy: their column names live in `Index`
   objects over numpy object arrays behind C extensions (pandas 3.0.6 is 636k lines of
   Python plus 45 `.so` modules), so following the source yields "open" where the model
   yields the columns, and the one precision measurement on large dispatch-heavy code (the
   old 3.4k-line interpreter: 961 false positives) says the joins would swamp the result.
   Note on cost: it is not modules × per-module time — call sites that must decide
   something about their arguments tabulate the callee in the caller's budget, and time
   grows with call-graph depth, not lines (830 lines 0.08 s, 1.2k lines 1.5 s, 3.4k
   self-referential lines 69 s); a budget hit costs precision, not time.
8. **More examples** (as time allows), each specified before running. Six so far
   (`o98`–`o103`); three passed as written, three found the engine problems listed under
   "From the `o` cases". Four older expectations that said "possibly" where the program is
   decided (`g40`'s try/else, `g47`'s retry) now follow the program.

Status: steps 1–8 done (step 8 is open-ended) (commits `ea08dab`, `639ca1a`, `ecd773b`, `e95115d` and the
budget / context commit after it); no `#@todo` expectations remain — `ROWAN_TODO=check` makes
the harness check any that are added later.

Open engine items carried over: precision on self-referential programs (`register`-closure
pattern, dispatch over `self.objects` tables), the spec §15.4 runtime oracle on real
repositories, the PyO3 wheel on another toolchain, `j93`'s open checks (classes propagated
through summary bindings).
