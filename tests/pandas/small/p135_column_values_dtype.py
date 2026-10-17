"""A column's values as pandas stores them: ints beside a float are floats (`[1, 2.5]` holds
1.0), a missing number is NaN and makes the ints floats, a missing str is NaN; a column
mixing other kinds (bools beside numbers, None beside bools) keeps its values as given.
Iteration and `to_dict("records")` give them so. Rowan kept 1 for 1.0 when no value was
missing (`f"{x}"` was "1", Python prints "1.0").

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"i": [1, 2.5], "n": [1, None], "s": ["a", None], "o": [1, "a"], "b": [True, None]})
i0 = f"{list(df['i'])[0]}"
#@value i0: exactly={"1.0","2.5"}
#@todo value i0: exactly={"1.0"}  (the first value)
n0 = f"{list(df['n'])[0]}"
#@value n0: exactly={"1.0","nan"}
s1 = f"{list(df['s'])[1]}"
#@value s1: exactly={"a","nan"}
o0 = list(df["o"])[0]
#@value o0: exactly={1,"a"}
b1 = list(df["b"])[1]
#@value b1: exactly={True,None}
r0 = f"{df.to_dict('records')[0]['i']}"
#@value r0: exactly={"1.0","2.5"}
recs = pd.DataFrame([{"i": 1}, {"i": 2.5}])
q0 = f"{list(recs['i'])[0]}"
#@value q0: exactly={"1.0","2.5"}
