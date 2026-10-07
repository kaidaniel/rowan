# AGENTS.md — rowan, handoff notes

Static key-set inference for Python records: infers, for every dict-like value (dicts,
DataFrame columns, object attributes, kwargs, JSON/CSV rows, argparse namespaces…), which
keys it has at each point and which keys the program demands of it; demands on external
inputs become inferred schemas. Design: `SPECIFICATION.md` (17 sections, Appendix A golden
cases; §9.8 lists how the prototype departs from the text). User-facing summary: `README.md`.
This file is for whoever picks the work up.

## State of the code (Phase-0 Python prototype, ~8.5k lines)

| file | role |
| --- | --- |
| `rowan/domain.py` | `Row` (present/definite/likely/absent + sources with `Lens`es + mutators + symbolic terms/dterms/lterms/pstores), `KeyVal` (`Lit`/`Lits`/`KeysOf`/`Data`/`Bot`/`Param`), `classify()` → tiers, `chain_extend`. Rows are frozen; hash and `refs` cached. |
| `rowan/layered.py` | `LayerMap` / `TwoLevel`: persistent layered maps (O(1) copy, O(diff) join/same/diff_keys). |
| `rowan/state.py` | `Obj`, `State` (env, rows, slots, elems, reqs, ret, exits, `symbolic`, `narrowed`). |
| `rowan/scopes.py` | lexical scopes, cells, free variables. |
| `rowan/interp.py` | flow-sensitive abstract interpreter over the AST; calls, classes/MRO, loops, narrowing, models dispatch, entry-point analysis of uncalled functions. |
| `rowan/summary.py` | symbolic function summaries (`SummaryMixin`): placeholders, deferred accesses (`SymReq`), events + fallback, footprint (diff + GC), instantiation. |
| `rowan/models.py` | library models (builtins, dict/list/str, json/yaml, csv, argparse, os.environ, dataclasses, functools, requests, pandas). |
| `rowan/report.py` | per-access combination across targets, schema table, text/JSON. |
| `tests/run_golden.py`, `tests/cases/` | 36 golden cases; expectations are `#@ tier key` / `#@row` / `#@schema` comments. |
| `examples/etl_report.py` | realistic script with planted bugs (also golden case e34). |

Run: `python -m rowan FILE.py [--json --no-rows --no-summaries --no-entry --no-imports]`,
`python tests/run_golden.py [-v] [case…]` (36/36 pass, ~0.3 s). `ROWAN_SUMMARIES=0` = old
per-call-site engine (34/36: a10 expectation was tightened for the summary engine).
`ROWAN_DEBUG=1` prints entry-point progress, instantiation/tabulation statistics and slow
instantiations to stderr. Analysis runs in a thread with a 1 GiB stack (`rowan/__init__.py`).

## Engine, as it stands

Verdict tiers: ✓ proven, ✓~ likely (unknown code intervened), ⚠ possibly, ✗ definitely
missing, ⚠~ likely missing, ◌ depends on input (→ schema), ⚠data data-derived key into a
closed record; `UNKNOWN` targets drop the whole access from the report (used for `Any` fields).

Summaries (`summary.py`): a body is analysed once in a symbolic `State` where untouched
objects have `Row.entry(ρ)`; parameters/self/touched paths are placeholder objects (kind
`ph`, payload `PH`, kinds param/self/vararg/kwarg/path/narrow). A `Summary` holds the exit
footprint (`rows_fp/slots_fp/elems_fp` = exit−entry diff, minus identities, minus objects
unreachable from return/entry slots/module bindings), `symreqs` (accesses deferred to the
call site; merged by (node,key,oid,ns,guarded,chain[:1],kind,cls)), `rel_verdicts`, notes,
`templates` (cloned per call site, ctx = call site or `o<receiver>`), `events`. A call
site tabulates (concrete per-site analysis, the old engine) only when it has what an event
needs: call/dispatch through a placeholder bound to an actual object, literal for
narrow/truth/isinstance, literal container for iter. Budgets per function:
`MAX_TABULATIONS=12` (literal events), `MAX_CALL_TABULATIONS=32` (call, dispatch). Beyond
them the summary is used; symreqs carry the placeholder's class (`SymReq.cls`) so a method
reached by name-based dispatch is only applied to instances of its class.
Recursion: partial summaries, `MAX_SUMMARY_ROUNDS=3` with join-widening; a callee
summarised against a partial summary higher on the stack is cached as final (known
imprecision; SCC is not re-iterated). Closure summaries are specialised on the code their
cells hold (transitively); data cells holding instances of one class get that class.
Declared types are trusted: annotations, `self.x = C()` in the class body, dataclass
fields (`field_class`), `isinstance` guards (narrowed placeholders; `State.narrowed` for
attribute chains on concrete bases), bindings filtered by declared class. Module-level
dict rows are entry facts once the module has run (`module_done`). Fresh objects get strong
keys during `__init__` (`fresh_init`) and template rows are strong for new clones.

Measured (entry pass on, imports followed, false ✗ count in parentheses): golden suite
0.3 s; `scopes/layered/domain/report/state.py` 0.2–1.3 s (0); `summary.py` 29 s (0, 260 ⚠);
old engine on `state.py` was 28 s with 169 ✗. `models.py` and `interp.py` (self-analysis
that reaches the whole interpreter through `Call.an: Analyzer`) do not finish in 50 min;
an intermediate version did `models.py` in ~100 s with 0 ✗ before the declared-type and
isinstance precision work. Old engine: `models.py` 23 s with 11 ✗. `--no-entry` makes
`interp.py` 0.6 s.

## Known problems

- Self-analysis blow-up (above). Profiles show ~90 % of time in `_instantiate`: deferred
  accesses × target objects; placeholders rooted at `self` resolve to dozens of same-site
  clones (States, LayerMaps, their `*`-element unknowns). Mitigations already in:
  grouping targets by site when > `MAX_SYMREQ_TARGETS=8`, per-target concretisation only
  when the row mentions the placeholder, `is_entry` fast path. Switching off each recent
  feature individually did not restore the 100 s run; cause not isolated.
- Objects allocated in non-literal loops are weak (`single=False`): attributes stored
  outside `__init__` show as "⚠ set on some paths only". Recency abstraction would fix it.
- Mutual recursion uses stale callee summaries (see above); `State.same` ignores `narrowed`.
- Container element positions are weak (tuple unpacking from dict elements mixes slots),
  producing "depends on which object" ⚠ noise in the self-analysis reports.
- No `match` narrowing, no `getattr` with computed names, pandas ops with data-derived
  columns stay open (SPECIFICATION.md §16).

## Debugging aids used this session

Scratch scripts (not in the repo; easy to recreate): a driver with
`faulthandler.dump_traceback_later`, a stack sampler (`sys._current_frames` every 20 ms,
aggregated by own-code frame / phase), cProfile with `threading.Timer(DELAY, pr.enable)`
inside the analysis thread, and an inspector that stops the analysis after N seconds via
`PyThreadState_SetAsyncExc` and dumps `an.summary_table` statistics (symreq counts by
line/oid/chain). `on_stmt=` hook on `Analyzer` prints rows at chosen statements.

## Next steps (agreed direction: Rust port)

1. Port to Rust, Python prototype kept frozen as the oracle. Parser: `ruff_python_parser`
   / `ruff_python_ast`. Persistent maps: `im`/`rpds` or SPARTA's Rust Patricia trees.
   Expose via PyO3 + maturin so `tests/run_golden.py` can run both engines and diff
   verdict sets. Order: domain + layered/state (property-test against Python), scopes,
   interpreter core without models, models as the golden cases need them, summaries last.
2. Change, don't transcribe: intern keys/objects/sources/lenses to `u32`; key sets as
   sorted `SmallVec`/bitsets so join is a merge/OR; rows arena-allocated with structural
   sharing (spec §15).
3. Algorithmic fix independent of language: resolve a placeholder to *allocation sites*
   with one representative row each, splitting by object only when rows differ (same-site
   collapse at resolution time, not per access). This is what the self-analysis case needs.
4. Then: recency abstraction for loop allocations; proper SCC iteration for mutual
   recursion; `narrowed` in `State.same`/join; positional container slots.
5. Validate on real repositories (spec §15.4 differential runtime harness) before tuning
   report volume; current reports on the analyser's own modules are too noisy (⚠ breakdowns).