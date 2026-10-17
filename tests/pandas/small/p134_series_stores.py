"""What a column holds after stores into it: `s.loc[label] = v`, `.iloc`, `.at`, `s[label]`
(a Series), `df.loc[label, col] = v`, `.at`, `.iat` (a frame), `inplace=True` methods and
`update`: the values iterated afterwards include the stored ones. A store at a label not
there before adds a row (NaN in the other columns). Rowan kept the values the column was
made of.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
s2 = df["b"].copy()
s2.replace("x", "z", inplace=True)
v2 = list(s2)[0]
#@value v2: includes={"z"}
s3 = df["b"].copy()
s3[0] = "q"
v3 = list(s3)[0]
#@value v3: includes={"q"}
s4 = df["b"].copy()
s4.update(pd.Series(["w"]))
v4 = list(s4)[0]
#@value v4: includes={"w"}
s5 = df["b"].copy()
s5.loc[0] = "k"
v5 = list(s5)[0]
#@value v5: includes={"k"}
#@todo value v5: exactly={"k"}  (the cell at label 0)
s6 = df["b"].copy()
s6.iloc[1] = "m"
v6 = list(s6)[1]
#@value v6: includes={"m"}
s7 = df["b"].copy()
s7.at[0] = "t"
v7 = s7[0]
#@value v7: includes={"t"}
df.loc[0, "b"] = "u"
v9 = list(df["b"])[0]
#@value v9: includes={"u"}
df.at[1, "b"] = "v"
v10 = list(df["b"])[1]
#@value v10: includes={"v"}
df.iat[0, 1] = "w"
v11 = df.loc[0, "b"]
#@value v11: includes={"w"}
df.loc[5, "b"] = "n"
n12 = len(list(df["a"]))
#@value n12: includes={3}
a12 = list(df["a"])[0]
#@value a12: includes={1.0}
