# drop on an unannotated parameter given frames: what it returns is the parameter without those
# columns, tied to it: a column read after the drop is needed of the parameter on every path, a
# dropped column is needed (drop raises KeyError for a missing one, unless errors="ignore") and
# gone afterwards; dropping rows keeps the columns.
import pandas as pd


def trim(df, cut):
    if cut:
        df = df.drop(columns=["x"])
    return df["y"]
#@schema param trim.df: always={y} sometimes={x}
#@only param trim.df: x if cut


def keep(df):
    out = df.drop(["a", "b"], axis=1)
    return out["a"]                                  #@ missing a
#@schema param keep.df: always={a,b} only={}


def lenient(df):
    out = df.drop(columns=["z"], errors="ignore")
    return out["w"]
#@schema param lenient.df: always={w} only={}


def rows(df):
    out = df.drop(index=[0])
    return out["v"]
#@schema param rows.df: always={v} only={}


if __name__ == "__main__":
    f = pd.DataFrame({"x": [1], "y": [2], "a": [3], "b": [4], "w": [6], "v": [7]})
    trim(f, True)
    trim(f, False)
    try:
        keep(f)
    except KeyError:
        pass
    lenient(f)
    rows(f)
