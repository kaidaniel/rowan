"""`df.insert(loc, name, value)` puts the column at position loc (not at the end), and raises
ValueError for a name the frame already has. `squeeze()` of a 1 x 1 frame is its cell as a
numpy scalar, whose `tolist()` gives the Python value (an int has no `tolist`).

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
df.insert(0, "n", 0)
first = list(df.columns)[0]                              #@value first: exactly={"n"}
df.insert(2, "m", 1)
order = "|".join(df.columns)                             #@value order: exactly={"n|a|m|b"}
try:
    df.insert(0, "a", 5)
    dup = "inserted"
except ValueError:
    dup = "ValueError"
vd = dup                                                 #@value vd: exactly={"ValueError"}
one = pd.DataFrame({"a": [7]})
cell = one.squeeze()
try:
    as_py = cell.tolist()
except AttributeError:
    as_py = "AttributeError"
vp = as_py                                               #@value vp: includes={7}
