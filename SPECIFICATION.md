# rowan — an abstract interpreter for Python

Status: started as draft v0.1 of a design for *key-set inference on Python records*; the
engine that implements it became a general abstract interpreter (§0, §2), and this document
now describes it that way. Section numbers are kept (other documents cite them); open
questions are collected in §17.

---

## 0. Summary

**What it does.** `rowan` is a whole-program abstract interpreter for Python. Every
expression evaluates to an abstract *value*: a set of possibilities, each a literal
(`1`, `"a"`, `None`), an abstract heap object (a dict, list, tuple, set, instance, class,
function, closure, generator, iterator, module, exception, …) or *unknown* (`?`: data from
outside, a parameter nobody passes, the result of unknown code). Objects carry what is
known about their contents through *models*, one per kind of thing: the key sets of dicts
and the attributes of instances (§2.1), list and tuple positions, literal arithmetic and
comparisons, string operations, iterator cursors, dict insertion order, exception values,
and models of the standard library and pandas (§11). Control flow (branches, loops,
exceptions, `match`, `with`, generators, closures, calls) is interpreted over these values,
per call site where it matters (§9).

What it reports, from those values:

1. **What each name holds** at each point, in a notation that reads like Python (§2.2): `{"id": 1, "role"?: "admin", **?}`, `[1, ?]`, `list[Order(id=?)]`, `1 | 2 | ?`.
2. **Accesses that can fail** — a key or attribute that is definitely, possibly or likely missing, with tiers (§3.1).
3. **Requirements on inputs** — which keys and attributes the program demands of values that enter from outside (`read_csv`, `json.load`, request bodies, env, parameters of functions nobody calls), propagated back to where they came from: the inferred schema (§3.2).

Key sets were the starting point and remain the model behind 2 and 3 (§2.1): may-set,
must-set, closed/open, with every source of openness *attributed* to a program point. They
are not privileged otherwise: an unknown dict value, list element, attribute or literal is
the same `?`.

**How.** Four stages: (1) Python → small SSA IR with an explicit heap and closures-as-objects; (2) points-to + call graph (Andersen-style, with context spent only where it matters: 1-call-site heap cloning for factory allocations, 1-object sensitivity for receivers); (3) per function, two abstract-interpretation fixpoints over a *row lattice* whose elements are symbolic in the function's inputs, so that a function's fixpoint directly yields a polymorphic summary (a jump function) instantiated at every call site — the IDE framework in abstract-interpretation clothing; (4) library models supply row transformers for stdlib/pandas, and a reporter turns rows + requirements into verdicts.

**Key decisions.**
- D1 One value domain for everything; what is known about an object's contents comes from the model for its kind (§2). Key-addressed values (dicts, instances, DataFrame columns, namespaces, …) share one key-set model; types differ only in their models.
- D2 Facts live on abstract heap objects, not on variables; aliasing is a points-to question (§6).
- D3 Uncertainty is *attributed*, never collapsed to ⊤: "unknown function `lib.f` may have changed this" is carried alongside the optimistic key set, so a sound reading and a useful reading coexist in one value (§7, §10, §13).
- D4 Context sensitivity through summaries (functional approach), not call strings; heap identity is the only k-limited thing (§9, §14).
- D5 Soundness is relative to models and modulo Python's dynamic features — the same stance as pyright/mypy.

**Out of scope (v1).** Keys built by arbitrary string computation, `getattr`/`setattr` with computed names, `exec`, metaclass magic, exhaustive pandas coverage, must-information through data-dependent recursion, Series index semantics.

**Reading order.** §1–3 (what), §4–9 (how), §10–13 (edges and output), §14 (theory), §15 (implementation incl. SPARTA), §16–17 (limits, open questions), Appendix A (golden test cases).

---

## 1. Goals, non-goals, success criteria

**Goals**
- G1 Schema inference for external inputs (CSV/Excel/SQL/JSON/env/argparse/request bodies/API responses) from usage, split into *always-read* and *sometimes-read*, with renames traced back to the source.
- G2 Function documentation: for functions returning records, the returned keys (must/may/open) *as a function of the arguments' keys*.
- G3 Diagnostics: (a) definitely-missing key on a closed row (typos, after projection/deletion/merge-suffixing); (b) possibly-missing (conditional construction); (c) likely-missing after unknown code; (d) data-derived key indexing a closed record.
- G4 Developer-tool latency: whole-program first run in seconds for ~50k LOC; incremental re-analysis of a changed function and its dependents well under a second (engine phase, §15.3).
- G5 Precision where naive tools fail: closures, factories, object state, kwargs plumbing — no merging by variable name.

**Non-goals (v1).** Type inference as an end in itself (values are tracked as far as they matter for the reports and the notation); soundness against dynamic features beyond `exec`/`eval` of literal code; concurrency; pandas MultiIndex/row-label semantics.

**Success criteria (prototype).** Zero false ✗ on the golden suite (Appendix A) and on the differential runtime harness (§15.4); every runtime-observed key set contained in the sound concretization; report volume on real repos within a budget set after a baseline run (target: a handful per KLOC, majority actionable).

---

## 2. Values and models

A value is a set of possibilities: literals, abstract objects (§6.1) and possibly *unknown*.
An object's kind selects the models that describe its contents and behaviour: dicts and
instances the key-set model below; lists and tuples positions (known length, known prefix,
or any number of elements like `X`); generators and iterators a cursor; classes their
attributes, MRO and protocol methods (`__getitem__`, `__iter__`, `__bool__`, descriptors,
…); functions and closures their bodies and captured cells; library calls their models
(§11). Unknown is one thing for every kind — an unknown dict value, list element, attribute
or argument are all `?`.

### 2.1 The key-set model (records)

**Definition.** A *record* is any value addressable by a finite set of names. The key-set model manipulates triples `(object, namespace, key)`.

**Namespaces**
- `item` — `obj[key]` on dicts and dict subclasses, DataFrame columns, TypedDict, `os.environ`, `**kwargs`.
- `attr` — `obj.name` on instances, modules, dataclasses, NamedTuples, `argparse.Namespace`, pandas `itertuples()` rows.

`item` and `attr` are distinct namespaces on the same object. Models may alias them: DataFrame `df.col` ≡ `df["col"]` for identifier-like columns; `vars(o)` / `o.__dict__` expose `attr` as an `item` view of the same object.

**Not records** (other models describe them)
- Lists, tuples, sets: positions when known, else an element value (§2).
- Instances of user classes defining `__getitem__` / `__getattr__`: `obj[k]` lowers to a method call and requirements flow through the method body. `Mapping` / `UserDict` subclasses are analysed through their methods unless a model says otherwise.
- pandas Series: not a record (its index is row labels, i.e. data). Exceptions are the virtual rows produced by `iterrows` / `apply(axis=1)` / `itertuples`, which are records keyed by the frame's columns.

**Keys.** Literals of type `str`, `int`, `bool`, tuples of those, and enum members are tracked; anything else is `Data` (§5.4).

**Decision.** All type-specific semantics (what `merge`, `asdict`, `**` do to key sets) live in models (§11); the core never special-cases a type.

### 2.2 Notation for values

Values are printed as Python-like text (the text report, the editor hover, `--facts`):

| Shown | Meaning |
| --- | --- |
| `1`, `"a"`, `None` | a known literal |
| `1 \| 2 \| ?` | one of several possibilities (`?` listed last) |
| `?` | a value nothing is known about |
| `a`, `a(b)`, `cfg["db"]`, `rows[*].name`, `json.loads(text)`, `1 + b` | an unknown value named in the program's own terms: a parameter, the result of a call rowan cannot see into, a read from another unknown (`[*]`: any element), an input, an expression over unknowns. A symbol is printed only where its names mean the same thing (inside its function; module-level ones everywhere); elsewhere the value is `?`. The legend under a value says what is known about each symbol (`a: ?`, `cfg: {"db": ?, **?}`). |
| `{"id": 1, "role"?: "admin", "x"~: ?}` | a dict with exactly these keys: `"id"` always, `"role"` on some paths only (`?` after the key), `"x"` unless unknown code removed it (`~`) |
| `{"id": 1, **?}` | the same, plus possibly other keys (an unknown dict spread in: the record is open) |
| `Order(id=1, **?)` | an instance and its attributes, same marks |
| `[1, ?]`, `(1,)` | a list / tuple with known positions |
| `list[X]`, `set[X]`, `tuple[X, ...]` | any number of elements, each like `X` (`list[?]`: nothing known) |
| `{…}`, `[…]`, `Name(…)`, `…` | left out by the printer (depth, more than eight entries, a cycle) |

---

## 3. Outputs

### 3.1 Verdict tiers for an access

For each load / `del` / `pop` / `[]+=` site `o[k]`, per target object, then combined (§13):

| Tier | Condition | Reading |
| --- | --- | --- |
| ✓ proven | k ∈ definite.proven | sound |
| ✓ likely | k ∈ definite.likely ∖ definite.proven (an unknown mutator touched o after k was established) | optimistic |
| ⚠ possibly missing | k ∈ present ∖ definite — set on some path, not all; report names the store sites and their conditions | sound |
| ✗ definitely missing | row closed and k ∉ present, or k ∈ absent.proven | sound |
| ⚠ likely missing | k ∉ present and `open` contains only Mutator attributions — report names the calls that could have added it | optimistic |
| ◌ input-dependent | k ∉ present and `open` contains Adder attributions — folded into the schema report, not a diagnostic | — |
| ⚠ data key into record | k is `Data(…)` and o is a closed record (not a map, §12) | heuristic |
| (none) | o is a map; `.get`, `in`, guarded loads | — |

### 3.2 Schemas
For every open source (an allocation carrying an `Adder` attribution): `always ⊆ sometimes` key sets; per key, the access sites that demanded it; disjunction groups ("`email`: in `orders.csv` or `customers.csv`"); renames traced back (`customer_id` ← `id`).

### 3.3 Function summaries (human-readable)
```
build(user, admin) -> dict
  returns : {id} ∪ {role}?                  closed
  requires: user.attr ⊇ {id}
public(rec) -> dict
  returns : keys(rec) ∖ {password}
point(**kw) -> dict
  returns : {x, y} ∪ keys(kw)
```

### 3.4 Machine output (the JSON field names `row`, `present`, `definite`, `closed` describe the key-set model of §2.1)
JSON lines: `{"kind": "verdict" | "schema" | "summary", "site": "file:line:col", "object": "<stable id>", …}`. Object ids are `alloc@file:line:col[context]`, stable across runs for IDE integration.

---

## 4. Architecture

```
 .py files ──► Frontend (§5) ──► IR: SSA locals, CFG, explicit heap ops
                                      │
                                      ▼
                         Points-to + call graph (§6)
                                      │
                                      ▼
     ┌── per function, in call-graph SCC order (§8, §9) ──────────────────┐
     │   forward  : object ↦ RowExpr   (provision, symbolic in inputs)     │
     │   backward : object ↦ Req       (requirement)                       │
     │   summary  = exit RowExpr / entry Req per placeholder + return      │
     └──────────────────────────────────────────────────────────────────────┘
                                      │        ▲
                                      ▼        │  models (§11)
                                 Reporter (§13)
```

Everything upstream of the fixpoints is Python-specific. The fixpoints are generic lattice machinery (§14). Models are a registry.

---

## 5. Frontend and IR

### 5.1 Inputs
A package root and/or entry files. Imports are resolved against configured roots. Third-party and stdlib modules are not analysed unless pure-Python and opted in; they are covered by models or treated as unknown (§10). Type stubs (typeshed, pandas-stubs) are consulted only for *return kinds* (record / frame / container / scalar) and for declared parameter types used as points-to seeds (§6.3).

### 5.2 IR
Per function: a CFG of basic blocks; locals in SSA (phi at joins); the heap explicit.

```
v = const c                       # literal: str / int / bool / None / tuple of consts
v = alloc site kind               # kind ∈ {dict, list, set, tuple, obj(class), closure(fn), cell, virtual}
v = load   o ns k                 # k is an IR value; ns ∈ {item, attr}
    store  o ns k v
    delete o ns k
v = call   f args kwargs          # f is an IR value; *args / **kwargs flagged
v = phi    [(pred_block, v_i)]
    branch cond → true_block, false_block
    ret v
    raise v
v = iter   o kind                 # kind ∈ {keys, items, values, elems}; v is the loop variable
    guard  cond                   # marker on CFG edges for narrowing (§8.4)
```
Every instruction keeps its source position. Loads carry a `guarded` flag (§5.3).

### 5.3 Lowering rules

| Python | IR |
| --- | --- |
| `d = {"a": x, **e}` | `alloc dict`; `store d item "a" x`; copy `e → d` (model `dict.update`) |
| `d[k] = v`, `d[k]`, `del d[k]` | store / load / delete |
| `d[k] += v` | load then store |
| `o.a`, `o.a = v` (incl. `self`) | load / store with ns = attr |
| `{k: f(v) for k, v in src.items()}` | own scope; `alloc dict`; loop over `iter src items`; store |
| list/set comprehensions | own scope; container alloc; element stores |
| dict union operator (PEP 584), `{**d, **e}` | fresh dict + two copies |
| `for k in d`, `d.keys()`, `d.items()` | `iter d keys/items` |
| `for x in lst` | `iter lst elems` |
| `with ctx as v` | `v = call ctx.__enter__`; `__exit__` call on every exit edge |
| `@dec def f` | `f = call dec f0` |
| `lambda`, nested `def` | `alloc closure` with captured cells; free variables read/written through cells; `nonlocal` write = store to cell |
| `class C:` body | module-level `alloc obj(class)`; methods are functions with a `self` placeholder (§9.2) |
| `try: … except KeyError/LookupError/Exception/bare:` | loads in the `try` body get `guarded` |
| `assert cond`, `if cond` | `guard` on the outgoing edges |
| `functools.partial(f, a)` | model: closure-like object binding `a` |
| conditional expression, walrus, chained comparison, star-unpacking | standard desugaring |
| `global g` write | store to the module object's attr slot |
| `yield v` (v1) | element store into the generator's virtual container; `await` transparent |

### 5.4 Key values (string domain)
Every IR value has a `KeyVal`, its abstract value when used as a key, computed by constant propagation over SSA:

```
KeyVal ::= Lit(k)                 # one tracked literal
         | Lits(S)                # finite set, |S| ≤ 64: literal containers; loop variables over them
         | Param(p)               # the value of placeholder p of the enclosing function (resolved at call sites)
         | KeysOf(o, ns)          # keys of object o   (from `iter o keys/items`, `o.columns`, `o.to_dict()`)
         | ElemsOf(o)             # elements of a container object (from `iter o elems`)
         | Map(f, KeyVal)         # f ∈ {upper, lower, strip, title, prefix(s), suffix(s), replace(a,b), str, int}
         | Data(site)             # anything else
```
A KeyVal may be a bounded disjunction (≤ 4 atoms); overflow → `Data`. An f-string with one hole and literal surroundings yields `Map(prefix/suffix, KeyVal(hole))`; other f-strings → `Data`. Cross-module constants (`from consts import KEY`) resolve through the module object's attr slot. Patterns (v2): a bounded vocabulary of `prefix*` keys so that `d[f"col_{i}"]` can match `d["col_3"]` as "possibly".

**Decision.** `ast` (CPython) is the parser: it is the only one guaranteed to track the running language version. LibCST only if an editor integration needs lossless positions.

---

## 6. Points-to and call graph

### 6.1 Abstract objects
`AbstractObject = (allocation site, heap context)`, interned to `ObjectId`.

- **Sites.** Every `alloc`; every modelled call that returns a fresh record/frame/container (`read_csv@L`, `merge@L`); every open source; placeholders (§9.2); access-path placeholders under open sources (`cfg["db"]` when `cfg` is open), k-limited to depth 3.
- **Heap context — 1-call-site cloning.** An allocation inside function `f`, executed on behalf of call site `c` of `f`, gets context `c`: one clone per call site of the allocating function. Deeper call chains keep only the innermost call site (k = 1). Module-level allocations have the empty context.
- **Receiver context — 1-object.** Allocations inside a *method* are cloned per receiver object instead of per call site, so each `Cache()` site gets its own `self.store`. Combined rule: context = receiver object if the allocating function is a method, else the call site.
- **Single flag.** An object is *single* if its site executes at most once per context: the site is not inside a loop in its function, the context's call site is not inside a loop, and the allocating function is not in a recursive SCC. Only single objects receive strong updates.

### 6.2 Slots
`(ObjectId, ns, key) → PointsToSet` for literal keys; `(ObjectId, ns, *)` for non-literal keys; `(ObjectId, elem)` for containers; `(closure, cell_i)`; `(module, attr)`; `(class, attr)` for class attributes and methods; `(instance, attr)` falls back to the class MRO. Dicts are therefore field-sensitive on literal keys, so `cfg["db"]["host"]` reaches the right inner object.

Containers additionally record `elems : KeyVal` (join of their literal elements), so `["clicks", "views"]` gives `ElemsOf → Lits({clicks, views})`.

### 6.3 Resolution
Inclusion (Andersen) constraints over SSA values and slots, solved by a worklist; flow-insensitive on the heap, flow-sensitive on locals via SSA.

- `call f` with `f` pointing to function objects → direct edges; closures bind cells; bound methods bind `self`.
- `o.m(…)` → MRO lookup on the classes of `o`'s objects; several classes → several targets, joined at analysis time (§9.6).
- `C(…)` → fresh instance + `__init__`.
- Modelled callables → the model decides (may create virtual objects, may invoke callbacks with virtual arguments).
- Unresolvable (`getattr` with computed name, unknown module, dynamic) → *unknown call* (§10).
- Annotation seeds: a parameter annotated with class `C` points to a placeholder instance of `C`; `-> pd.DataFrame` makes the return value a frame object; TypedDict / dataclass annotations give the placeholder a *declared row* (§8.5).

### 6.4 Entry points
Functions never called inside the program (CLI mains, framework handlers, tests) are analysed with all parameters as open `Adder` sources, so their requirements become schemas (request bodies, CLI args). A config list of framework decorators (`@app.get`, `@celery.task`, …) marks handlers explicitly; the heuristic default is "uncalled and decorated".

---

## 7. Abstract domains

### 7.1 Attributions
```
Attr ::= Adder(site) | Mutator(site) | Top
```
- `Adder` — a source that can only *add* unknown keys: `read_csv@L`, `json.load@L`, a store with a `Data` key at L, an entry-point parameter, `**` of an open dict.
- `Mutator` — unknown code that could add *or remove* keys: an unknown call at L, an escape at L (§10).
- `Top` — nothing known (the lattice ⊤).

### 7.2 Row (provision)
For one object at one program point:

```
Row = ⟨ present  ⊆ K            keys with positive evidence (may)
        definite ⊆ present      keys present on all paths      — a pair (likely ⊇ proven)
        absent   ⊆ K            keys known removed / excluded  — a pair (likely ⊇ proven)
        open     ⊆ Attr         ∅ ⇒ closed row ⟩
```
`proven` variants are what sound reasoning may use; `likely` variants survive unknown mutators. They coincide until a `Mutator` attribution is added, at which point both `proven` sets become ∅ while `likely` is kept; later strong stores/deletes add to both again.

- **Order / join** (componentwise): `present` ∪, `open` ∪, every `definite`/`absent` variant ∩. ⊥ = unreachable (flag). ⊤ = `⟨∅, ∅, ∅, {Top}⟩`.
- **Sound concretization.** `γ(r) = { S | definite.proven ⊆ S, S ∩ absent.proven = ∅, open = ∅ ⇒ S ⊆ present }`. When `open ≠ ∅`, `present` is not part of γ; it is evidence used by the optimistic reading, which substitutes `likely` for `proven` and treats `present` as the candidate set with `open` as the list of things that could violate it.
- **Invariants** maintained by every transfer: `definite ⊆ present`; `present ∩ absent = ∅`. `absent` only carries information when `open ≠ ∅` (a closed row is absent everywhere outside `present` by definition).

### 7.3 RowExpr (symbolic rows, for summaries)
`RowExpr = Row + terms`, where terms reference placeholders ρ (§9.2):

```
terms_present  : { ρ ↦ (kill ⊆ K, rename) }     contributes rename(ρ.present  ∖ kill)
terms_definite : { ρ ↦ (kill ⊆ K, rename) }     contributes rename(ρ.definite ∖ kill); placeholders ⊆ those of terms_present
rename         : finite partial map K → K  |  named transformer (upper, lower, prefix(s), …)  |  identity
```
Meaning: `present(r) = present ∪ ⋃_ρ rename_ρ(ρ.present ∖ kill_ρ)`; likewise for `definite`; `open(r) = open ∪ ⋃_ρ ρ.open`.

**Transfers** (literal parts are pointwise; the terms column states the extra effect):

| transfer | literal sets | terms |
| --- | --- | --- |
| strong store k | present ∪{k}; definite(both) ∪{k}; absent(both) ∖{k} | — |
| weak store k | present ∪{k}; absent.likely ∖{k} | — |
| store with key `Data(site)` | open ∪{Adder(site)} | — |
| strong delete k | present ∖{k}; definite ∖{k}; absent(both) ∪{k} | every ρ: kill ∪{k} |
| weak delete k | definite(both) ∖{k} | — |
| rename m | m applied to present/definite; absent := ∅ unless closed | rename := m ∘ rename |
| project to C (`df[[…]]`, `usecols`, `df.columns = …`) | present := C; definite(both) := C (projection raises on missing); open := ∅ | terms := ∅ |
| copy from r₂ (`{**r₂}`, `dict(r₂)`, `update`, merge/concat side) | present ∪ present₂; definite ∪ definite₂ (strong) or unchanged (weak); absent ∖ present₂, and := ∅ if open₂ ≠ ∅; open ∪ open₂ | terms ∪ terms₂ |
| mutator at L | open ∪{Mutator(L)}; proven variants := ∅ | — |

**Join of RowExprs.** Literal parts as in Row. `terms_present`: union over placeholders; for a shared ρ, `kill = kill₁ ∩ kill₂`; differing renames are kept as a term list (cap 4), beyond that degrade to `open ∪ {Top-rename@L}`. `terms_definite`: intersection over placeholders, `kill = kill₁ ∪ kill₂`, renames must agree.

**Composition is substitution.** Instantiating ρ with a RowExpr r′ yields a RowExpr (the terms of r′ become terms of the result with composed kills/renames). The class is closed under substitution and join — exactly the condition the IDE framework places on jump functions (§14).

### 7.4 Req (requirement, backward)
```
Req     = ⟨ always ⊆ KeyTerm, sometimes ⊆ KeyTerm ⟩,  always ⊆ sometimes
KeyTerm ::= Lit(k) | Param(p) | Map(f, KeyTerm) | ElemsOf(ρ) | KeysOf(ρ) | Data(site)
```
Join (backward merge, i.e. at CFG splits): `always` ∩, `sometimes` ∪. Backward transfers at a node, for object o:

| node | effect on Req(o) |
| --- | --- |
| unguarded load / delete / `pop` without default `o[k]` | sometimes ∪{k}; always ∪{k} if k is a single term |
| guarded load, `.get`, `in`, `setdefault`, `pop(k, default)` | — |
| strong store `o[k] = v` | always ∖{k}; sometimes ∖{k} |
| weak store | always ∖{k} |
| `o₂` derived from `o` by copy / rename / projection (model edge) | Req(o) ∪= rename⁻¹(Req(o₂)) |
| `o₂` derived from `o` and `o′` by union (merge, concat) | sometimes(o) ∪= Req(o₂); plus a disjunction-group id for the report; `on`-keys go to `always` of both |
| call | callee Req over placeholders, `Param(p)` substituted by the call-site KeyVal of the argument, transferred to the actual objects |
| allocation / open source | terminal: schema entry (open) or discharged (closed; redundant with the forward pass) |

### 7.5 State
Forward state at a program point: `ObjectId ↦ RowExpr` (absent key ⇒ ⊤; the whole map is ⊥ at unreachable points). Backward state: `ObjectId ↦ Req` (absent key ⇒ no requirement).

---

## 8. Intraprocedural analysis

### 8.1 Forward pass
Abstract interpretation over the CFG with the transfers of §7.3, applied to the objects an instruction touches according to points-to. A store through a value that points to several objects, or to a non-single object, is weak; otherwise strong. Loads narrow (§8.4). Fixpoint: Bourdoncle weak topological ordering + worklist. The lattice is finite, so no widening is needed for termination; the caps on term lists and KeyVal disjunctions are the only widenings.

### 8.2 Backward pass
The CFG reversed, with the transfers of §7.4. Runs after the forward pass of the same function (it needs strong/weak classification, guard flags and narrowing). Requirements reaching a placeholder become part of the function's Req summary; those reaching a local allocation are discharged (closed) or become schema entries (open).

### 8.3 Loops
- General loops: ordinary fixpoint. Stores in a loop are weak unless the target object was allocated in the same iteration (the single flag is tracked with loop depth).
- **Exhaustive literal loops.** `for v in e` where `KeyVal(e) ∈ {Lits(S), ElemsOf(o) with elems = Lits(S)}`, `S ≠ ∅`, and the body executes `store o[v]` on every iteration (the store dominates the back-edge; no `break` / `continue` / `return` / `raise` before it): on the loop's normal-exit edge apply a strong store of every `k ∈ S`. This is what makes `{f: 0 for f in ["a", "b"]}` a closed, definite row without unrolling.

### 8.4 Narrowing (guards)
Edge transfers on the true/false CFG edges, applied to every object the tested value points to (definite form only for single objects; otherwise present-only):

| condition (true edge) | effect |
| --- | --- |
| `k in o`, `o.get(k) is not None`, `o.get(k)` truthy, `k in df.columns`, `hasattr(o, k)` | definite(both) ∪{k} |
| `k not in o`, or the false edge of the row above | present ∖{k}; absent(both) ∪{k} |
| `S <= o.keys()`, `set(S).issubset(o)`, `all(k in o for k in S)` with `Lits(S)` | definite ∪ S |
| `assert cond` | as `if cond` with the false edge unreachable |
| successful unguarded load `o[k]` (fall-through edge) | definite(both) ∪{k} — "if execution continued, k was there" |
| load inside `try … except KeyError` | guarded: no requirement; no narrowing on the handler path |
| `isinstance(o, dict)`, TypedDict narrowing | v2 |

The fall-through rule is how requirement facts feed provision facts; the merge-suffix case (Appendix A, case 9) depends on it.

### 8.5 Declared rows
TypedDict, dataclass, NamedTuple, pydantic and `attrs` definitions yield a *declared row*: `present = definite = fields`, closed; `total=False` / optional fields are present-only; pydantic `extra="allow"` → open `Adder`. Used (a) as the entry row of placeholder parameters annotated with them, (b) as a check against the inferred return row of functions annotated `-> T` ("returns {…} but declares T"), (c) by the `asdict()` / `vars()` models.

---

## 9. Interprocedural analysis

### 9.1 Order
Condense the call graph into SCCs; process bottom-up. Inside an SCC, iterate all member summaries to a fixpoint, starting from ⊥ summaries (returns ⊥, no effects, no requirements). Bounded by lattice height; 2–3 rounds in practice.

### 9.2 Placeholders
A summary is expressed over placeholders ρ, one for each of:
- parameter (positional, keyword-only; `*args` as a container; `**kwargs` as a dict record),
- captured cell (closures),
- module global read or written,
- `self`, and every access path from a placeholder that the body touches (`self.store`, `cfg["db"]`), k-limited to depth 3,
- the receiver's class attributes.

On entry each placeholder object has `RowExpr = ⟨∅, ∅, ∅, ∅⟩ + terms {ρ ↦ (∅, identity)}` — "whatever the caller passes".

### 9.3 Summary
```
Summary(f) = { placeholder-rooted path ↦ RowExpr at exit }        effects
           ∪ { return ↦ RowExpr | fresh object(s) }
           ∪ { placeholder-rooted path ↦ Req at entry }           requirements
           ∪ { escaping allocation sites }                        cloned per call site on instantiation
           ∪ { calls through placeholders }                       callbacks, resolved at instantiation
```

### 9.4 Instantiation at a call site
1. Bind placeholders to the caller's objects (points-to at the call site) and `Param(p)` to the arguments' KeyVals.
2. Substitute into each effect RowExpr; apply to the caller's state — strong (replace) if the placeholder bound to exactly one single object, else weak (join with the existing row).
3. Create or look up clones of escaping allocations with context = this call site; bind the return value's RowExpr.
4. Substitute into the Req summary and add to the caller's backward state.
5. **Aliasing.** If two placeholders bind to overlapping object sets, the summary's no-alias assumption is violated; v1 downgrades every effect through either placeholder to weak. (v2: re-analyse the callee under the aliasing pattern, cached per pattern.)

### 9.5 Closures, callbacks, higher-order calls
A closure object carries its cells (§6.2). A call through a value that points to closure objects instantiates the inner function's summary with its cell placeholders bound to those cell objects — which were cloned per creation call site (§6.1), so two counters from one factory never share a row. Models that invoke callbacks (`map`, `sorted(key=…)`, `apply(axis=1)`, `pipe`, `groupby.apply`) instantiate the callback's summary with model-supplied virtual arguments (e.g. a row object whose RowExpr copies the frame's). Decorators: analysable ones are ordinary calls; `wraps`, `lru_cache`, `property`, `staticmethod`, `classmethod`, `contextmanager` have models; an unknown decorator makes calls to the decorated function unknown in the sound reading and identity in the optimistic one.

### 9.6 Methods and object state
`self` is a placeholder; attribute paths from it are placeholders; effects on them are part of the summary (`put`: `self.store` gains `Param(k)`). At `c.put("user", u)`, `self.store` binds to the dict allocated in `__init__` for the receiver object of `c` (1-object context), and the update is strong if that dict is single. Dynamic dispatch to several candidate methods joins their instantiated effects and intersects their `always` requirements.

### 9.7 Recursion
Handled by the SCC fixpoint. The return RowExpr converges to the join over all return sites; objects allocated in recursive calls collapse onto one clone per call site (k = 1), so self-referential heap shapes stay finite. Must-information through recursion is lost by construction (Appendix A, case 10).

### 9.8 Implementation notes (written for the Phase-0 prototype; the Rust engine in `rowan/` behaves the same except where `AGENTS.md` says otherwise — the prototype itself has been removed)
What the prototype does where it departs from the text above; see README "Deviations".
- **Order (9.1).** Summaries are computed on demand at the first call rather than bottom-up over precomputed SCCs. A function reached again while its own summary is being computed uses the *partial* summary of the previous round (⊥ in round 1) and the function iterates (≤ 6 rounds) until its return value, exit state and deferred accesses are stable. A callee summarised against a partial summary of a function further up the stack is cached as final — the SCC is not re-iterated as a whole (known imprecision for mutual recursion).
- **Fallback (9.3/9.4).** Besides effects and requirements, a summary records *events*: places where the symbolic analysis lost information about a placeholder (a call or attribute dispatch through it, `isinstance`/truthiness/`in` on it, iteration over it). A call site whose binding for that placeholder carries the missing information (an actual function or object; a literal; a literal container) analyses the body concretely instead of instantiating the summary — always for calls/dispatch, at most 12 times per function for the literal/structural events. Beyond that budget the summary is used; accesses it recorded under an `isinstance` guard or a declared class are then applied only to targets of that class.
- **Footprint (9.4).** The effects kept are the exit/entry *difference* of the symbolic heap (identity rows and slots holding their own entry placeholder are dropped) restricted to objects the caller can reach (return value, entry objects' slots, module bindings); the rest of the body's allocations are dead at exit. When a placeholder binds to several caller objects, its exit RowExpr is substituted per target with that target's own entry row (not the join), which keeps rows from growing across aliased calls.
- **Context inside a summary.** Deferred accesses keep one call site of context below the function; the caller's chain is prepended on use, and a recursive instantiation of a partial summary does not nest the recursive site, so the number of distinct contexts stays bounded.
- **Closures (9.5).** A closure's summary is specialised on the code its cells hold (sibling closures, classes); data cells stay symbolic.
- **Declared types (8.5).** A parameter annotated with a user class, a `self.x = C(...)` assignment in the class body or a dataclass field `x: C` gives the placeholder a class: methods resolve on it, name-based dispatch is avoided, and objects of unrelated classes are filtered out of its binding. Fields annotated `Any`/`object` make accesses through them unreportable.

---

## 10. Unknowns, escape, attribution

**Unknown call at L** (no summary, no model):
- every object reachable (k-limited, through slots) from the arguments and receiver gets `Mutator(L)` (§7.3 last row);
- the return value is a fresh virtual object with `Row = ⟨∅, ∅, ∅, {Mutator(L)}⟩` for every record-like return kind the stubs allow (dict / frame / unknown); scalar kinds produce no object.

**Escape at L.** Storing an object into a module global, a class attribute, or an object passed to unknown code, or returning it from an entry point: `Mutator(L)` from that point on — not retroactively; facts computed earlier in the same function stand.

**Allowlist (config).** Callees known not to mutate record arguments (`print`, `logging.*`, `len`, `json.dumps`, …) add nothing. Everything with a model is handled by the model.

**Readings.** The sound reading of a row with a `Mutator` attribution is `⟨definite.proven, absent.proven, open⟩`; the optimistic reading uses `present` and the `likely` variants. §13 fixes which tier uses which.

---

## 11. Library models

### 11.1 Interface
```python
class Model(Protocol):
    def apply(self, call: CallSite, args: BoundArgs, state: ForwardState) -> Effects: ...

class Effects:
    edges:     list[tuple[ObjectId, Transfer]]          # transfers of §7.3, incl. copy-from
    returns:   RetSpec                                   # Fresh(RowExpr) | Alias(arg) | NoRecord | Unknown
    requires:  list[tuple[ObjectId, KeyTerm, bool]]      # (object, key, always) — consumed by the backward pass
    callbacks: list[tuple[Value, list[VirtualArg]]]      # callable argument + the arguments to call it with
```
A declarative layer covers most models: `identity`, `copy(src)`, `gen(keys)`, `kill(keys)`, `rename(map | fn)`, `project(keys)`, `open(Adder)`, `require(arg, keys)`, `callback(arg, row_of=other_arg)`, selected per argument shape (`Lits` vs `Data`). Models are keyed by resolved qualified name plus receiver kind (`pandas.DataFrame.merge`), with a per-kind fallback (`pandas.DataFrame.*` → `copy + Adder(L)` for unmodelled methods that return a frame), so the long tail degrades to "open", not to "wrong".

### 11.2 Initial registry

**Builtins / stdlib.** `dict.{get, setdefault, update, pop, popitem, keys, values, items, copy, clear, fromkeys}`; `dict()`; `dict(zip(ks, vs))` (`Lits` → closed); `{**}` / PEP 584 union; `copy.copy` (new outer object, inner slots aliased); `copy.deepcopy` (fresh recursively); `json.load(s)` (open `Adder`, nested access-path placeholders); `json.dump(s)` (reads: no requirement); `csv.DictReader` (rows: open `Adder` per reader; `fieldnames=` → closed); `os.environ` (open `Adder`, program-wide); `argparse` (`add_argument("--x")` → attr `x` on the parsed namespace); `dataclasses.asdict` / `fields`; `vars`; `getattr(o, Lit)`; `collections.defaultdict` / `Counter` (map role, never raise); `functools.{partial, wraps, lru_cache}`; `sorted` / `map` / `filter` / `min` / `max` with `key=`; `itertools.groupby`.

**pandas** (`df` is a DataFrame):

| call | row effect | requirement |
| --- | --- | --- |
| `read_csv/excel/parquet/sql/json(…)` | fresh frame, open `Adder(L)`; `usecols=Lits` or `names=Lits` → closed | — |
| `pd.DataFrame({"a": …})` | closed {a, …}; `pd.DataFrame(list_of_records)` → present = ∪, definite = ∪ (NaN-filled) | — |
| `df[Lit]`, `df.Lit` | Series (no record) | Lit |
| `df[Lits]` (list subscript) | fresh frame, `project(Lits)` | Lits |
| `df[mask]`, `df.loc[mask]`, `head/tail/sample/sort_values/drop_duplicates/fillna/dropna/astype/copy/reset_index(drop=True)` | copy (same row) | `sort_values(by=…)`: by |
| `df[Lit] = v`, `df.loc[…, Lit] = v`, `df.insert(…, Lit, v)` | strong/weak store Lit | — |
| `df.assign(a=…, b=…)` | copy + gen {a, b} | — |
| `df.drop(columns=Lits)`, `drop(Lits, axis=1)`, `del df[Lit]`, `df.pop(Lit)` | kill | Lits |
| `df.rename(columns=map or fn)`, `add_prefix`, `add_suffix` | rename | — |
| `df.merge(o, on=k, how=…)`, `pd.merge` | fresh: copy(df) ∪ copy(o); a non-key column `c` definite in both → gen `c_x`, `c_y`, kill `c` (custom `suffixes=` honoured); overlap only possible → gen `c_x`, `c_y`, keep `c` as present-only | k (or `left_on` / `right_on`) on each side |
| `df.join(o, lsuffix=, rsuffix=)` | as merge | — |
| `pd.concat([…], axis=0)` | union; definite = ∪ (NaN-filled) | — |
| `pd.concat([…], axis=1)` | union | — |
| `df.set_index(Lit)`, `reset_index()` | kill Lit / gen the index name when known | Lit |
| `df.groupby(k)[v].agg(f)`, `.sum()`, … | Series or frame with columns {v…}, index k | k, v |
| `df.groupby(k).agg(name=(col, f), …)` | closed {name, …} | k, cols |
| `df.groupby(k).agg({"c": f})` | closed {c, …} | k, c |
| `df.groupby(k).apply(fn)`, `df.pipe(fn)` | callback(fn, row_of=df); result from fn's summary | k |
| `df.apply(fn, axis=1)`, `df.iterrows()`, `df.to_dict("records")` | callback / virtual item-record with df's RowExpr | — |
| `df.itertuples()` | virtual attr-record with df's RowExpr | — |
| `for c in df`, `df.columns`, `df.to_dict()` | `KeysOf(df)` | — |
| `df.columns = Lits` | project(Lits) | — |
| `df.query("…")`, `df.eval("…")` | copy / gen | column names parsed from the literal (v2) |
| `df.pivot / pivot_table / melt / stack / unstack / T / get_dummies / crosstab / explode`, `pd.json_normalize` | fresh frame, open `Adder(L)`; `melt` with literal `id_vars / var_name / value_name` → closed | literal args |
| `df.filter(items=Lits)` / `filter(like=…)` | project / open | — |
| `df.values`, `to_numpy`, `shape`, `len`, `describe()` | no record | — |
| unmodelled `pandas.DataFrame.*` returning a frame | copy + `Adder(L)` | — |

### 11.3 Model tests
Every model has a 3-line runtime oracle test: build concrete inputs, run pandas / the stdlib, compare the real key set against the model's row (§15.4).

---

## 12. Role classification (noise control)

An object's *role* is computed from the forward result at the end of the allocating function and refined by callers:
- `record` — at least one literal key store and no `Data`-keyed store;
- `map` — at least one `Data`-keyed store (or `defaultdict` / `Counter` / `dict.fromkeys(Data)`) and at most one literal store;
- `mixed` — both (a config dict with a literal `"default"` plus data keys): record for literal keys, map for `Data` keys;
- open sources are records (their keys are external, not computed).

`Data`-keyed loads on a `map` are not reported. On a closed `record` they are reported once per (object, load site) as "data key into closed record" with a suggestion (`.get`, validation).

---

## 13. Reporting

Per access site, per target object: derive the tier from §3.1 using `proven` variants for ✓ / ✗ and `likely` variants for the "likely" tiers. Combine over the points-to targets: all ✓ → ✓; all ✗ → ✗; otherwise ⚠ with the per-target breakdown. KeyVal handling: `Lit` direct; `Lits` → the worst key, listed; `KeysOf(o₂)` → ✓ if `present(o₂) ⊆ definite(o)`, ⚠ otherwise; `Data` → §12.

Explanations: for "possibly missing", the store sites of k (a side table filled during the forward pass) with the nearest enclosing condition of each; for "definitely missing" after projection / deletion / merge-suffixing, the responsible site (every kill records its site). Deduplication: one report per (object, key, tier) per function; schema entries aggregate program-wide.

---

## 14. Theory and complexity

- **Lattices.** The forward domain (§7.3) and backward domain (§7.4) are complete lattices of finite height: keys range over `K` (program literals), attributions over program points, placeholders over a function's inputs, term lists are capped. All transfers are monotone, so Kleene iteration from ⊥ reaches the least fixpoint (Tarski) in at most *height* steps; Bourdoncle's WTO orders the worklist so nested loops converge in few sweeps. No widening is needed for termination; the caps are precision/efficiency widenings.
- **Distributivity → IDE.** Every transfer distributes over join except the merge-suffix and intersection-shaped comprehension models (over-approximated). The problem is therefore an IDE instance (Sagiv–Reps–Horwitz 1996): facts = objects, values = rows, jump functions = `rename(ρ ∖ kill) ∪ gen`, a class closed under composition and join (§7.3). Consequences: (1) the summary-based solution equals the meet-over-all-valid-paths solution — full call-site context sensitivity for rows without call strings (Sharir–Pnueli's functional approach); (2) polynomial cost: per function `O(|CFG| × |live objects| × WTO sweeps)`, interprocedurally `O(Σ_f |Summary(f)| × |call sites of f|)`; (3) demand-driven variants exist (run the backward pass from one source).
- **Where approximation lives.** Heap identity: k = 1 call-site cloning and 1-object receivers (Milanova-style object sensitivity), allocation-site summaries in loops and recursion, weak updates, a single `elem` slot per container. Nothing else merges.

---

## 15. Implementation plan

### 15.1 Phase 0 — Python prototype (validate the design)
Frontend (`ast`) → IR → points-to → forward/backward passes → reporter, with ~40 models, run on the golden suite (Appendix A) and then on 5–10 real repositories. Pure Python with interned `KeyId` / `ObjectId` ints and `frozenset`s. This handles tens of thousands of lines and lets the IR lowering and the models — the real risk — iterate daily. Deliverable: a CLI emitting §3.4 JSON.

### 15.2 Phase 1 — Evaluation
Differential runtime harness (§15.4); precision and noise metrics; model-coverage report (calls that degraded to `Adder` / `Mutator`, ranked by frequency); fix verdict thresholds; freeze the IR format.

### 15.3 Phase 2 — Engine
Keep the frontend in Python (§5.4 decision). Serialize the IR (flatbuffers or JSON lines) and move points-to and the fixpoints into a compiled engine.

**SPARTA (C++) / sparta-rs (Rust) assessment.** The fit is good for the fixpoint layer and nowhere else.

- *Direct mappings.* `Row` = reduced product of an over/under set domain (`present` / `definite`; `PatriciaTreeOverUnderSetAbstractDomain` in C++), an under-set domain (`absent`), a powerset (`open`), using `ReducedProductAbstractDomain`; `RowExpr` adds a small map domain for terms. Per-point state = `PatriciaTreeMapAbstractEnvironment<ObjectId, RowExprDomain>`. Fixpoint = `MonotonicFixpointIterator` over our CFG with `analyze_node` / `analyze_edge` — edge transfers are exactly §8.4 narrowing; the backward pass uses the backwards iteration adaptor; SCC levels run in parallel on `WorkQueue`. Patricia trees keyed by interned ints give persistent, structure-sharing environments, which matters once a function's per-node states hold hundreds of objects.
- *Not provided.* The Python frontend, points-to / call graph, summaries and instantiation, models, the reporter — i.e. most of the code. Mariana Trench is the template for the interprocedural layer (bottom-up per-method models over "ports", k-limited access paths, "features" ≈ our attributions) and Pysa for Python-specific call resolution; both are worth reading before writing §9.
- *IDE vs SPARTA is not a choice.* Running SPARTA's intraprocedural fixpoint over the *symbolic* RowExpr domain computes the function's jump function at the exit node; summaries compose bottom-up as in Mariana Trench. No tabulation algorithm is needed, and nothing in §7–9 changes.
- *C++ vs Rust.* The Rust port is younger and ships fewer combinators (confirm over/under sets, environments and the backwards adaptor; each is small to write). The Python-tooling ecosystem is Rust: `ruff_python_parser` if the frontend ever leaves Python, PyO3/maturin to ship a wheel, salsa for incremental recomputation of summaries. C++ if the team already runs C++/Buck or wants to lift Mariana Trench code directly. Recommendation: Rust, after Phase 1 has proven the design; the engine is under a third of the system.

### 15.4 Testing
- **Golden suite.** Appendix A cases as fixtures with exact expected rows, requirements and verdicts.
- **Differential runtime oracle.** Instrument programs (a `sys.settrace` hook plus wrappers on `dict`-equivalent access and `DataFrame.__getitem__`) to record the concrete key set at each access site during test runs. Every observed set must lie in the sound concretization of the inferred row — a violation is a bug or a model gap — and the optimistic reading's precision is measured against the same data.
- **Model oracle tests** (§11.3).
- **Noise budget.** Tier counts per KLOC on real repositories, with manually labelled samples.

### 15.5 Phase 3 — Integration
LSP (hover: row and requirements; diagnostics), CI mode (schema drift between commits), pre-commit. Incremental: summaries cached by function body hash + model-registry version; dependents invalidated through the call graph.

---

## 16. Known limitations
- Keys from string computation outside the `Map` vocabulary → `Data`, unless the parts are known literals (the engine evaluates `%` / f-string formatting, `split`, `partition`, `join` on known values).
- `getattr` / `setattr` with computed names, `exec`, `__getattr__` / `__getitem__` magic on unmodelled classes, metaclasses → unknown.
- One element slot per container when positions are unknown: a list of heterogeneous dicts joins their rows (may-only). The engine keeps positions for literal containers, straight-line appends, decided comprehensions and known-key sorts.
- Module-global state mutated from many places: flow-insensitive join → may-only.
- Depth-dependent recursion: no must-information (case 10).
- Unknown decorators and framework-driven dispatch: unknown in the sound reading; mitigated by the entry-point rule (§6.4) and models.
- pandas operations whose columns come from data are open by construction; Series index semantics ignored.
- Must-information across weak updates and summary objects is lost; strong updates need the single flag (§6.1).
- The optimistic reading is deliberately unsound and is labelled as such in every output.

---

## 17. Open questions for review
1. §2 — DataFrame attribute aliasing: only for identifier-like columns, or always?
2. §6.1 — k for heap cloning: 1 call site proposed; 2 would separate `make_handler(cfg)` called from two wrappers of one factory. Decide on Phase 1 data.
3. §7.2 — Keep `present` as labelled evidence in schema reports under the sound reading, or drop it there?
4. §7.4 — Disjunctive requirements from union sources: side-table groups (proposed) or a disjunction term in `Req`?
5. §8.3 — Loop-unrolling budget (64) and the policy for comprehensions over `Data`.
6. §6.4 — Entry-point detection: config list, heuristics, or both?
7. §9.4 — Aliasing fallback: weak-everything (proposed) vs cached re-analysis.
8. §9.5 — Treat unknown decorators as identity in the sound reading too? (Proposed: no.)
9. §2 — Series: ignore index entirely in v1? (Proposed: yes.)
10. §15.1 — Emit the IR in the frozen serialized format from day one, so the Phase 2 engine consumes prototype output unchanged?

---

## Appendix A — Golden cases

Each case lists the code and the exact expected facts. `R(o)` is the row of object `o` at the marked point; `Req(src)` the schema arriving at an open source.

### A1. Conditional key, closed row, narrowing
```python
def build(user, admin):
    d = {"id": user.id}
    if admin:
        d["role"] = "admin"
    return d

r = build(u, flag)
r["role"]                        # ⚠ possibly missing — store at L4 under `admin`
if "role" in r: r["role"]        # ✓ proven (narrowed)
r.get("role", "user")            # no requirement
r["rolle"]                       # ✗ definitely missing — closed row
try: r["tenant"]                 # guarded — no verdict, no requirement
except KeyError: ...
```
Expect: `Summary(build).return = ⟨present {id, role}, definite {id}, absent ∅, open ∅⟩`; `Req(user.attr) = {id}` always.

### A2. Unknown calls vs open sources
```python
cfg = json.load(fp)              # R = ⟨∅, ∅, ∅, {Adder(json.load@L1)}⟩
cfg = lib.apply_defaults(cfg)    # unknown: return object R = ⟨∅,∅,∅,{Mutator(@L2)}⟩; arg gets Mutator(@L2)
cfg["timeout"]                   # ◌ schema: json@L1 needs timeout — "unless apply_defaults adds it"

opts = {"host": "db", "port": 5432}
lib.apply_defaults(opts)         # in-place unknown: open += {Mutator(@L7)}, proven := ∅, likely kept
opts["host"]                     # ✓ likely
opts["user"]                     # ⚠ likely missing — only apply_defaults@L7 could add it
```
Expect: `Req(json@L1) = {timeout}` sometimes; attribution of the Mutator on the schema line.

### A3. One summary, three contexts
```python
def point(**kw):
    return {"x": 0, "y": 0, **kw}      # Summary: return = ⟨{x,y}, {x,y}, ∅, ∅⟩ + terms {ρ_kw ↦ (∅, id)}

point(z=1)["z"]                         # ✓ proven
point()["z"]                            # ✗ definitely missing
point(**json.load(fp))["z"]             # ◌ schema: that json needs z
```
Expect: the summary is computed once; three distinct return clones (`alloc@L2[call@L4]` etc.).

### A4. Loop keys: literal vs data
```python
d = {}
for c in ["a", "b", "c"]:               # exhaustive literal loop → strong stores on exit
    d[c] = compute(c)
d["a"]                                   # ✓ proven
d["z"]                                   # ✗ definitely missing

counts = {}
for w in words:                          # Data(words@L8)
    counts[w] = counts.get(w, 0) + 1     # role: map
counts["the"]                            # not reported (map)
```
Expect: `R(d) = ⟨{a,b,c}, {a,b,c}, ∅, ∅⟩` after the loop; `R(counts) = ⟨∅, ∅, ∅, {Adder(store@L9)}⟩`, role map.

### A5. Relational rows
```python
def public(rec):
    return {k: v for k, v in rec.items() if k != "password"}   # return terms {ρ_rec ↦ ({password}, id)}
def shout(rec):
    return {k.upper(): v for k, v in rec.items()}               # return terms {ρ_rec ↦ (∅, upper)}

u = {"name": "k", "password": "…"}
public(u)["password"]            # ✗ definitely missing
shout(public(u))["NAME"]         # ✓ proven
shout(public(u))["name"]         # ✗ definitely missing
public(json.load(fp))["x"]       # ◌ schema: json needs x
```
Expect: `Summary(public).return` has no literal keys, one term; instantiation with `R(u)` gives `⟨{name}, {name}, ∅, ∅⟩`.

### A6. Escaping closures
```python
def make_counter(fields):
    counts = {f: 0 for f in fields}          # cell object; dict cloned per call site of make_counter
    def bump(name): counts[name] += 1        # Req(counts) = {Param(name)} always
    return bump, (lambda: dict(counts))      # dict(o): copy edge

bump, snap = make_counter(["clicks", "views"])
bump("clicks")                               # ✓ proven
bump("purchases")                            # ✗ definitely missing in this counter
other, _ = make_counter(["purchases"])
other("purchases")                           # ✓ proven — separate cell, separate row
snap()["views"]                              # ✓ proven
```
Expect: two cell objects `cell@L2[call@L6]`, `cell@L2[call@L9]`; `R` of the first dict `⟨{clicks, views}, {clicks, views}, ∅, ∅⟩` via `ElemsOf(ρ_fields)` → `Lits`.

### A7. Object state
```python
class Cache:
    def __init__(self): self.store = {}
    def put(self, k, v): self.store[k] = v       # effect: self.store gains Param(k) (strong if single)
    def user(self): return self.store["user"]    # Req(self.store) = {user} always

c = Cache(); c.user()                            # ✗ definitely missing
c.put("user", u); c.user()                       # ✓ proven — strong update on a single object
d = Cache(); d.user()                            # ✗ definitely missing — separate instance
```
Expect: `self.store` placeholder bound to `dict@L2[obj c]` vs `dict@L2[obj d]`.

### A8. Aliasing is a points-to question
```python
a = {"x": 1}
b = a;       b["y"] = 2;  a["y"]                 # ✓ proven — same object
c = dict(a); c["z"] = 3;  a["z"]                 # ✗ definitely missing — copy
n = {"inner": a}
copy.copy(n)["inner"]["w"] = 1;     a["w"]       # ✓ proven — shallow copy shares the inner object
copy.deepcopy(n)["inner"]["q"] = 1; a["q"]       # ✗ definitely missing
```

### A9. pandas pipeline
```python
orders = pd.read_csv(f_orders)                                      # open: Adder(read_csv@L1)
cust = pd.read_csv(f_cust).rename(columns={"id": "customer_id"})    # open; rename edge

orders["total"] = orders["qty"] * orders["price"]                   # Req: qty, price; strong store total
WEIGHTS = {"EU": 1.0, "US": 1.2}
def score(row):
    return row["total"] * WEIGHTS[row["region"]]
orders["score"] = orders.apply(score, axis=1)   # callback with virtual row ← R(orders): Req region flows back
                                                # ⚠ data key into closed record: WEIGHTS[Data(orders.region)]
orders["name"].str.strip(); cust["name"].str.strip()   # fall-through narrowing: name ∈ definite of both
m = orders.merge(cust, on="customer_id", how="left")   # Req customer_id on both; name → name_x, name_y; kill name
m["email"]                                              # ◌ email ∈ ρ_orders ∪ ρ_cust (disjunction group)
m["name"]                                               # ✗ definitely missing — suffixed by merge@L13
top = m[m["score"] > 1][["customer_id", "email", "name_x"]]   # project: closed
top["qty"]                                              # ✗ definitely missing after projection
```
Expect schemas: `Req(read_csv@L1) = {qty, price, region, name, customer_id}` always, `{email}` sometimes (group with cust); `Req(read_csv@L2) = {id, name}` always (`customer_id` mapped back through the rename), `{email}` sometimes.

### A10. Recursion
```python
def tree(depth):
    if depth == 0: return {"leaf": True}
    return {"l": tree(depth - 1), "r": tree(depth - 1)}

t = tree(3)
t["l"]["r"]["leaf"]            # ⚠ possibly missing at every step
t["x"]                          # ✗ definitely missing
```
Expect: `Summary(tree).return = ⟨{leaf, l, r}, ∅, ∅, ∅⟩` after two SCC rounds; the inner objects are the same clone (`dict@L3[call@L3]`).

### A11. Dispatch
```python
class Base:
    def enrich(self, d): return d
class A(Base):
    def enrich(self, d): return {**d, "a": 1}
class B(Base):
    def enrich(self, d): return {**d, "b": 1}

def run(h: Base): return h.enrich({"id": 1})
run(A())["a"]                          # ✓ proven — receiver resolved to A.enrich
run(B())["a"]                          # ✗ definitely missing
[h.enrich({"id": 1}) for h in hs]      # hs unknown: join over MRO targets → ⟨{id, a, b}, {id}, ∅, ∅⟩
```
