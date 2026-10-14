# merge with overlapping columns: the left frame's columns in order (overlaps with the left
# suffix), then the right frame's (overlaps with the right suffix). The suffixed pair was
# put side by side at the left column's place.
import pandas as pd

df = pd.DataFrame({"city": ["a", "b"], "sales": [1.0, 2.0], "qty": [1, 2]})
other = pd.DataFrame({"city": ["a", "b"], "pop": [10, 20], "sales": [3.0, 4.0]})
m = df.merge(other, on="city", suffixes=("_l", "_r"))
cols = list(m.columns)
second = cols[1]                                 #@value second: exactly={"sales_l"}
third = cols[2]                                  #@value third: exactly={"qty"}
fifth = cols[4]                                  #@value fifth: exactly={"sales_r"}
d = df.merge(other, on="city")
last = list(d.columns)[-1]                       #@value last: exactly={"sales_y"}
