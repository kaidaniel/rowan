"""Selections and removals of columns a frame lacks raise: `df[["a", "zz"]]` KeyError,
`del df["zz"]` KeyError, `df.columns = [...]` with another number of names ValueError,
`df.query("zz > 1")` a NameError (pandas' UndefinedVariableError), `df.loc[:, "z"] = 1` on a
frame of no rows ValueError. Inside a `try`, the handler runs.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
try:
    sel = list(df[["a", "zz"]].columns)
except KeyError:
    sel = "KeyError"
vs = sel                                                 #@value vs: exactly={"KeyError"}
try:
    del df["zz"]
    d = "deleted"
except KeyError:
    d = "KeyError"
vd = d                                                   #@value vd: exactly={"KeyError"}
try:
    df.columns = ["x", "y", "z"]
    c = "renamed"
except ValueError:
    c = "ValueError"
vc = c                                                   #@value vc: exactly={"ValueError"}
try:
    q = len(df.query("zz > 1"))
except NameError:
    q = "NameError"
except KeyError:
    q = "KeyError"
vq = q                                                   #@value vq: exactly={"NameError"}
empty = pd.DataFrame()
try:
    empty.loc[:, "z"] = 1
    e = "set"
except ValueError:
    e = "ValueError"
ve = e                                                   #@value ve: exactly={"ValueError"}
