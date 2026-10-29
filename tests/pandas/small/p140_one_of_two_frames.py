"""A frame that is one of two (by a branch): each operation runs on each of them. A column's
values, a selection with `.loc`, `groupby(...).sum()`, `concat` with another frame: on the
path with the one-column frame the result has one column, on the other two. Rowan made one
result for both, as if every frame had every column.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import sys

import pandas as pd

small = len(sys.argv) > 5
v = pd.DataFrame({"a": [1]}) if small else pd.DataFrame({"a": [2], "b": ["z"]})
vals = v["a"].tolist()
first = vals[0]                                          #@value first: exactly={1, 2}
picked = v.loc[:, "a"].tolist()
p0 = picked[0]                                           #@value p0: exactly={1, 2}
g = v.groupby("a").sum()                                 #@row g: present={b} definite={} closed
both = pd.concat([v, pd.DataFrame({"q": [1]})])          #@row both: present={a,b,q} definite={a,q} closed
n = len(both.columns)                                    #@value n: exactly={2, 3}
