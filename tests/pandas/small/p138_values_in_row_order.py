"""A frame built from literal data knows its values in row order: `list(df["b"])`,
`df.loc[0, "b"]`, `df.iloc[1]["b"]` and `s.iloc[0]` are single values. Anything that
reorders or relabels the rows (`sort_values` / `sort_index` in place or not, `df.index =`,
`s.index =`, a reversed slice) keeps the values but not their order, so those reads are
one of the column's values again.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"a": [2, 1], "b": ["x", "y"]})
first = list(df["b"])[0]
#@value first: exactly={"x"}
cell = df.loc[1, "b"]
#@value cell: exactly={"y"}
row1 = df.iloc[1]["b"]
#@value row1: exactly={"y"}
last = df["b"].iloc[-1]
#@value last: exactly={"y"}
nums = list(df["a"])
n0 = nums[0]
#@value n0: exactly={2}
d1 = pd.DataFrame({"a": [2, 1], "b": ["x", "y"]})
d1.sort_values("a", inplace=True)
s1 = list(d1["b"])[0]
#@value s1: exactly={"x","y"}
d2 = pd.DataFrame({"a": [2, 1], "b": ["x", "y"]})
d2.index = [1, 0]
s2 = d2.loc[0, "b"]
#@value s2: exactly={"x","y"}
col = pd.DataFrame({"a": [2, 1]})["a"]
col.sort_values(inplace=True)
s3 = list(col)[0]
#@value s3: exactly={1,2}
lab = pd.DataFrame({"b": ["x", "y"]})["b"]
lab.index = [1, 0]
s4 = lab[0]
#@value s4: exactly={"x","y"}
d5 = pd.DataFrame({"a": [2, 1], "b": ["x", "y"]}).sort_values("a")
s5 = d5.iloc[0]["b"]
#@value s5: exactly={"x","y"}
d6 = pd.DataFrame({"a": [2, 1], "b": ["x", "y"]}).iloc[::-1]
s6 = list(d6["b"])[0]
#@value s6: includes={"y"}
#@todo value s6: exactly={"y"}  (the rows reversed: an .iloc slice keeps no values)
d7 = pd.DataFrame({"a": [2, 1], "b": ["x", "y"]})
d7.sort_index(ascending=False, inplace=True)
s7 = d7.iloc[0]["b"]
#@value s7: exactly={"x","y"}
