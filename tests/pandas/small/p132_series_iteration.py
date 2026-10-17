"""Iterating a Series gives its values, not its index labels (a dict gives its keys): `for`,
`list`, `sorted`, `set`, `max`, `zip`, `enumerate` and `str.join` over a column see what the
column holds, as Python scalars. Its keys, `in` and `len` are its index's. Rowan iterated a
column like a dict: its labels (`sorted(df["b"])` was `[0, 1]`).

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})

for v in df["a"]:
    w = v
#@value w: exactly={1,2}
names = list(df["b"])
first = names[0]
#@value first: exactly={"x","y"}
#@todo value first: exactly={"x"}  (the column's values in order)
srt = sorted(df["b"])
lo = srt[0]
#@value lo: exactly={"x","y"}
#@todo value lo: exactly={"x"}  (sorted values)
top = max(df["b"])
#@value top: exactly={"x","y"}
#@todo value top: exactly={"y"}  (the largest of the values)
vs = set(df["a"])
has1 = 1 in vs
#@value has1: includes={True}
#@todo value has1: exactly={True}  (both values are surely in the column)
has0 = 0 in vs
#@value has0: exactly={False}
n = len(df["a"])
#@value n: exactly={2}
lab = 0 in df["b"]
#@value lab: exactly={True}
val = "x" in df["b"]
#@value val: exactly={False}
for a, b in zip(df["a"], df["b"]):
    pa = a
    pb = b
#@value pa: exactly={1,2}
#@value pb: exactly={"x","y"}
for i, b2 in enumerate(df["b"]):
    pi = i
    pb2 = b2
#@value pi: exactly={0,1}
#@value pb2: exactly={"x","y"}
joined = ",".join(df["b"])
#@value joined: includes={"x,y"}
#@todo value joined: exactly={"x,y"}  (the values in order)
labels = list(df["b"].keys())
lab0 = labels[0]
#@value lab0: exactly={0,1}
#@todo value lab0: exactly={0}  (the index in order)
tl = df["a"].tolist()
t0 = tl[0]
#@value t0: exactly={1,2}
#@todo value t0: exactly={1}  (the column's values in order)
