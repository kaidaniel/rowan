# pd.cut gives NaN for a value outside its bins: a float, so str() of that cell is "nan"
# (rowan took the missing label for None, and str() of it for "None": the value set left out
# what the program prints). Expectations written from the program text.
import pandas as pd

df = pd.DataFrame({"s": [50.0, 75.0, 120.0]})
df["g"] = pd.cut(df["s"], bins=[0, 60, 101], labels=["F", "A"], right=False)
out = [str(r["g"]) for r in df.to_dict("records")]
g2 = out[2]
#@value g2: includes={"nan"}
g0 = out[0]
#@value g0: includes={"F"}
