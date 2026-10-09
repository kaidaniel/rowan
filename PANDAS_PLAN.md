# pandas model vs real pandas (and a runtime oracle for every case) — plan

The torch technique applied to the pandas model (SPECIFICATION §11.2), as §11.3 / §15.4
always intended: hand-written expectations in runnable programs, checked against real
pandas by a runtime oracle, then the engine fixed until every case passes. One commit per
step, only when the golden suite, snapshots and `cargo test` pass. Delete this file when done.

## What is checked (rowan's claims, the existing directives)

Rowan's pandas model makes key-set claims about **columns**: which a frame has on every path
(`definite`), on some (`present`), surely not (`absent`), whether others are possible
(`closed`/`open`); verdicts on column accesses; schemas (what `read_csv` inputs must hold).
The oracle (`tests/pandas_oracle.py`) runs each case under pandas and checks, after each
directive's line, in every run that reaches it:

* `#@row NAME: …` — the keys `K` of NAME's value (a frame's columns, a dict's keys, an
  `iterrows` row's index, an `itertuples` row's fields, an object's attributes):
  `definite ⊆ K`, `absent ∩ K = ∅`, and `K ⊆ present` when `closed`.
* `#@value NAME: exactly={…}` — the value is one of these.
* `#@ ok k` — no run raised a KeyError / AttributeError for `k` at that line; `#@ missing k`
  — every run reaching the line raised one; `#@ possibly k` — some run raised one and some
  did not (the driver shows both).
* `#@schema read_csv@L<n>: always={…} only={…}` — mutation: the oracle reruns the program
  with `read_csv` at line n returning the frame without one column; dropping an `always`
  column must make the program raise a KeyError for it, dropping a column outside
  `only`/`sometimes` must not.

Other tiers (`likely`, `input`, `datakey`, `none`, `sound`, …) and row floors are static
claims the oracle does not check.

## Steps

0. [x] This plan; harness: `tests/pandas/{small,models}` with `PENDING` (like torch), every
   directive on a comment-only line applies to the code line above (also `#@row` / `#@ `);
   `tests/oracle.py` (rows, values, verdicts, schemas; one subprocess per case with a timeout
   and a scratch working directory) with two modes: **strict** for `tests/pandas/` (every
   directive runs, no crash) and **lenient** for `tests/cases/` (the 417 plain-Python cases,
   written before the oracle: every claim a run reaches is checked, unreached ones are only
   counted); `tests/pandas/CONVENTIONS.md` (directives, rowan's pandas semantics, drivers with
   several inputs per function).
0b. [x] Run the lenient oracle on `tests/cases/`; every refuted claim is an engine bug (or a
   misread program): fix them.
1. [x] Cases, expectations by hand, each agreeing with the oracle: 100 small programs (one
   API area each) and 15 larger programs (realistic pipelines: ETL, reporting, feature
   engineering, joins of several sources, groupby reports, time series, …). All `PENDING`.
   They cover the modelled API (§11.2) **and APIs not modelled yet** (`where`/`mask`,
   `combine_first`, `stack`/`unstack`, `pivot`, `rolling`/`resample`/`expanding`,
   `str`/`dt` accessors, `map`/`replace`, `nunique`/`value_counts`, `astype` with dicts,
   `rename_axis`, `droplevel`, `merge_asof`, `squeeze`, `interpolate`, `shift`/`diff`, `cumsum`,
   `clip`, `round`, `isin`, `between`, `idxmax`, `corr`, `to_records`, `from_records`,
   `DataFrame.agg` with lists / dicts, `groupby(...).transform/filter/size/nth/first`, …),
   which then get models.
2. [x] Fix the engine until `PENDING` is empty (soundness first: a claim the oracle refutes
   is a bug; then precision).
3. [ ] A round of one-hypothesis cases on the weak spots found, fixed the same way.
4. [x] Docs (README, SPECIFICATION §11.2 / §11.3, AGENTS.md); delete this plan after step 3.

## Notes / decisions log

* The lenient oracle on the 417 plain cases: all agree (1,102 claims checked, 979 not reached:
  uncalled functions, missing input files, runs ending at a deliberate failure). A line with
  several tiers for one key (`#@ ok k ; missing k`, verdicts per call) is checked as "each
  holds in some run".
* Found while writing cases: `tests/cases/g55_pandas_more.py` claims `flags["flag"]` ok after
  `df.apply(flag, axis=1)` with `flag` returning a dict — pandas gives a Series of dicts, not a
  frame (the case never ran: its input file does not exist). Engine bug + wrong expectation.
