"""What a frame's columns hold after it changes in place: `df[mask] = v`, `df.columns = […]`
(the values go with the positions), `df.update(other)`, `inplace=True` methods and
`df.col = v` on an existing column. Rowan kept the values the frame was made of (and so
did `to_dict("records")`).

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

d1 = pd.DataFrame({"a": [1, 2], "c": [3, 4]})
d1[d1["a"] > 1] = 0
v1 = list(d1["a"])[1]
#@value v1: includes={0}
#@todo value v1: exactly={0}  (row 1 matched the mask)
d2 = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
d2.columns = ["b", "a"]
v2 = list(d2["a"])[0]
#@value v2: exactly={"x","y"}
#@todo value v2: exactly={"x"}  (the first value of the second column)
d3 = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
d3.update(pd.DataFrame({"b": ["z", "w"]}))
v3 = d3.to_dict("records")[0]["b"]
#@value v3: includes={"z"}
d4 = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
d4.replace("x", "r", inplace=True)
v4 = list(d4["b"])[0]
#@value v4: includes={"r"}
d5 = pd.DataFrame({"a": [1, 2], "b": ["x", "y"]})
d5.b = "attr"
v5 = list(d5["b"])[0]
#@value v5: includes={"attr"}
#@todo value v5: exactly={"attr"}  (the column is replaced)
