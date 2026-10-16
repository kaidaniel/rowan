# A missing value in a column of numbers or strs is NaN (a float; str() of it is "nan"), not
# None: `DataFrame({"a": ["x", None]})` stores NaN, ints with a missing value are floats, and a
# record without a key gives NaN. A column of other values (bools, mixed) keeps None.
# `astype(str)` keeps a missing value missing (pandas 3), so `get_dummies` drops it.
# rowan stored None (str() "None"), and astype(str) made "nan" a column. Expectations written
# from the program text and pandas' documented behaviour.
import pandas as pd

df = pd.DataFrame({"a": ["x", None], "b": [1, None], "c": [True, None]})
recs = df.to_dict("records")
a1 = str(recs[1]["a"])
#@value a1: includes={"nan"}
b0 = str(recs[0]["b"])
#@value b0: includes={"1.0"}
b1 = str(recs[1]["b"])
#@value b1: includes={"nan"}
c1 = recs[1]["c"] is None
#@value c1: includes={True}
d = pd.get_dummies(df["a"].astype(str))
print(d["x"])  #@ ok x
try:
    print(d["nan"])  #@ missing nan
except KeyError:
    pass
rows = pd.DataFrame([{"k": "u", "n": 2}, {"k": "v"}])
n1 = str(rows.to_dict("records")[1]["n"])
#@value n1: includes={"nan"}
