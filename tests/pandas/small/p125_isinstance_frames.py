# isinstance against pandas classes: a frame is a DataFrame, a column a Series; a dict or an
# instance of the program's own class is neither (main said False for the frame and the column).
import pandas as pd


class Rec:
    pass


df = pd.DataFrame({"a": [1, 2], "b": [3, 4]})
is_df = isinstance(df, pd.DataFrame)            #@value is_df: exactly={True}
is_ser = isinstance(df["a"], pd.Series)         #@value is_ser: exactly={True}
col_df = isinstance(df["a"], pd.DataFrame)      #@value col_df: exactly={False}
d_df = isinstance({"a": 1}, pd.DataFrame)       #@value d_df: exactly={False}
r_df = isinstance(Rec(), pd.DataFrame)          #@value r_df: exactly={False}
both = isinstance(df, (pd.Series, pd.DataFrame))  #@value both: exactly={True}
