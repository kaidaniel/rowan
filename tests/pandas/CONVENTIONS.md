# Writing pandas cases

Cases in `small/` (one API area each, 4–12 checks, 20–80 lines) and `models/` (realistic
pipelines, 120–300 lines, 20+ checks). Expectations are written **by hand from the program
text and pandas' documented behaviour, before rowan is run**; rowan is never used to produce
or adjust them. Each case must agree with the runtime oracle (strict mode):

```
uv run --no-project --with pandas python tests/oracle.py tests/pandas/small/p001_example.py
```

New cases go into `tests/pandas/PENDING` until the engine passes them.

## What rowan claims about frames

Rowan tracks **which columns** a value has — the same key-set model as for dicts (rows are
keys, values are not tracked for frames). Values are not printed in directives; only columns.
For a DataFrame the keys are its columns; for a dict its keys; for an `iterrows()` row (a
Series) its index (= the frame's columns); for an `itertuples()` row its fields; for an object
its attributes. A Series in general is *not* a record (its index is data).

Notation in `#@row NAME: present={…} definite={…} [absent={…}] closed|open`:
* `present` — every column that may be there (on some path);
* `definite` — the columns there on every path;
* `absent` — columns known not to be there (e.g. dropped, renamed away);
* `closed` — no other columns possible; `open` — other columns may exist (data from input,
  unknown code).

Rowan's model — use it to decide what rowan *should* say:
* `pd.read_csv(…)` (also `read_json/excel/parquet`): **open**, nothing known; with
  `usecols=[…]` or `names=[…]` closed with exactly those.
* `pd.DataFrame({"a": …, "b": …})`: closed {a, b}; from a list of record dicts: present =
  definite = the union (missing cells are NaN).
* A **successful access narrows the key in**: after `df["x"]` runs on an open frame, `x` is
  present and definite from there on (the program went on, so it existed).
* `df[["a", "b"]]` closed {a, b}; `df[mask]`, `head`, `tail`, `sample`, `sort_values`,
  `drop_duplicates`, `fillna`, `dropna`, `astype`, `copy`, `query`: same columns.
* `df["c"] = v`, `df.loc[:, "c"] = v`, `df.insert(i, "c", v)`, `df.assign(c=…)`: adds `c`
  (definite; only present when done on one branch).
* `drop(columns=…)`, `drop(…, axis=1)`, `del df[…]`, `df.pop(…)`: removes (absent).
* `rename(columns={…})` / a function, `add_prefix`, `add_suffix`: renames.
* `merge`/`join`: union of both; an overlapping non-key column `c` on both sides becomes
  `c_x`/`c_y` (or the given `suffixes`).
* `pd.concat([...])` (rows): union; `axis=1`: union.
* `set_index("k")` removes `k`; `reset_index()` adds the index name back (`index` when unnamed).
* `groupby(k).agg(name=(col, f))` closed {name}; `.agg({"c": f})` closed {c}; the group key
  becomes the index (add it back with `reset_index()` / `as_index=False`).
* `iterrows()` rows and `apply(f, axis=1)` rows: the frame's columns; `to_dict("records")`
  dicts: the frame's columns.
* pivot / melt / stack / get_dummies / json_normalize / anything whose columns come from
  data: open (melt with literal names: closed).
* An API rowan does not model (yet) returning a frame: rowan should still be *sound*: write
  what is **true** (the precise answer when it is derivable from the program text); rowan may
  fail today — that is the point (the model gets added).

## Verdicts on accesses (`#@ tier key`, on the access's line)

* `ok` — the column is surely there; `missing` — surely not (a closed frame without it, a
  dropped column); `possibly` — there on some paths only; `input` — depends on the data (an
  open frame from `read_csv`); `likely` / `likely-missing` — only unknown code could have
  changed it; `none` — no verdict; `sound` — anything but `missing`.
* Several on one line: `#@ ok a ; missing b`. A line run from several calls may get different
  verdicts per call: list each (`#@ ok k ; missing k`).
* The oracle checks `ok` (never raised), `missing` (always raised; wrap the access in a
  function the driver calls inside `try: … except (KeyError, AttributeError): pass`) and
  `possibly` (the driver must show both: one run where it is there, one where it raises).

## Schemas (`#@schema read_csv@L<n>: always={…} sometimes={…} only={…}`)

What the program requires of the input read at line n: `always` — needed on every path;
`sometimes` — on some; `only` — no other column is required. The oracle reruns the program
with that `read_csv` returning the frame minus one column: dropping an `always` column must
make the program raise a KeyError / AttributeError for it; dropping a column outside
`only`/`sometimes`/`always` must not. Give `read_csv` real data: `pd.read_csv(io.StringIO(CSV))`
with `CSV` a module-level string (or a `.csv` file next to the case).

## Program shape

* Functions with directives, plus a driver under `if __name__ == "__main__":` that runs
  **every** function **with several different inputs** (2–4: different column sets, empty
  frames, extra columns, values that take different branches). Every claim inside a function
  must hold for all of them — rowan's answer for a function is the join over its calls; the
  oracle checks each run. Module-level code also counts.
* Every directive line must run (strict oracle); the program must not crash (catch deliberate
  failures in the driver).
* Put the directive at the end of the statement's line, or on a comment line of its own
  directly below (it applies to the code line above). No directive inside a lambda's line.
* Keys: bare words or Python literals (`"Amount Due"`, `0`). Values: `#@value n: exactly={3}`.
* One-line header comment saying what the case probes.
