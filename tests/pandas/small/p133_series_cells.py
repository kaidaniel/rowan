"""A cell (`.loc[r, c]`, `.at[r, c]`, `.iat[i, j]`, `s[label]`, `s.iloc[i]`, a row's
`row["c"]`) is one value of the column, not a Series; a str column's cells are its str
values. An int column's cells are numpy scalars (`np.int64(1)`, not the int 1). The
truth value of a Series is ambiguous: `if s:` raises ValueError. Rowan took `.loc[r, c]`
and `.at` for a Series and `if s:` for a dict's truth.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})

c1 = df.at[1, "b"]
#@value c1: exactly={"y"}
c2 = df.loc[0, "b"]
#@value c2: exactly={"x"}
c3 = df.loc[0, "a"]
#@value c3: unknown
c4 = df.iat[0, 1]
#@value c4: exactly={"x"}
s = df["b"]
s0 = s[0]
#@value s0: exactly={"x"}
s1 = s.iloc[1]
#@value s1: exactly={"y"}
r = df.iloc[0]
rb = r["b"]
#@value rb: exactly={"x"}
ra = r["a"]
#@value ra: unknown
for _, row in df.iterrows():
    rb2 = row["b"]
#@value rb2: exactly={"x","y"}
for t in df.itertuples():
    tb = t.b
#@value tb: exactly={"x","y"}

try:
    if df["a"]:
        truth = 1
    else:
        truth = 2
except ValueError:
    truth = 3
after = truth
#@value after: exactly={3}
