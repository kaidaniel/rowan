"""Renaming a column to the name of another gives a frame with that name twice:
`list(df.columns)` is `["b", "b"]`, not `["b"]`. A frame of no rows: `apply(f, axis=1)` does
not raise what f raises on a row (it gives the empty frame back), and a `groupby` by a list of
as many names as there are rows, not all of them columns, groups by that list.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
cols = list(df.rename(columns={"a": "b"}).columns)
n = len(cols)                                            #@value n: types={int}
#@todo value n: exactly={2}  (a frame's columns are a set of names: the count is not known)
empty = pd.DataFrame()
try:
    out = empty.apply(lambda r: r["a"], axis=1)
    kind = type(out).__name__
except KeyError:
    kind = "KeyError"
vk = kind                                                #@value vk: includes={"DataFrame"}
frame = pd.DataFrame({"a": [1.5, None], "c": [True, False]})
try:
    sizes = frame.groupby(["a", "b"]).size().tolist()
    s = len(sizes)
except KeyError:
    s = "KeyError"
vs = s                                                   #@value vs: includes={2}
