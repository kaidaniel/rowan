"""`df.index = [...]` gives the frame new row labels (`df.loc["p"]` works, the old labels 0
and 1 are gone); `s.index = [...]` a Series new labels for the same values; `s.name = x`
names the column `to_frame()` makes. Rowan ignored the assignments: `df.loc["p"]` was
reported as failing on every path, and `s.to_frame()` kept the old name.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
df.index = ["p", "q"]
r = df.loc["p"]  #@ ok p
c = df.loc["p", "b"]  #@ ok p
#@value c: exactly={"x","y"}
#@todo value c: exactly={"x"}  (the cell at label "p")
keys = list(df["b"].keys())
k0 = keys[0]
#@value k0: exactly={"p","q"}
#@todo value k0: exactly={"p"}  (the index in order)


def old_label(frame):
    try:
        return frame.loc[0]  #@ missing 0
    except KeyError:
        return None


gone = old_label(df)
s = df["b"].copy()
s.index = ["m", "n"]
names = list(s.keys())
#@row s: present={m,n} definite={m,n} closed
s.name = "renamed"
fr = s.to_frame()
#@row fr: present={renamed} definite={renamed} closed
