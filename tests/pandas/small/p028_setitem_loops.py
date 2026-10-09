# df[...] = ... in loops: over a literal list, f-string names over range, a parameter list (maybe empty), each existing column, a while loop
import pandas as pd


def add_named(df):
    for name in ["x", "y", "z"]:
        df[name] = 0
    n = len(df.columns)                            #@row df: present={a,x,y,z} definite={a,x,y,z} closed
    return df["z"]                                 #@ ok z


def add_lags(df):
    for i in range(3):
        df[f"lag{i}"] = df["a"].shift(i + 1)       #@ ok a
    n = len(df.columns)                            #@row df: present={a,b,lag0,lag1,lag2} definite={a,lag0,lag1,lag2} closed
    return df["lag2"]                              #@ ok lag2


def add_from(df, names):
    for name in names:
        df[name] = 1
    n = len(df.columns)                            #@row df: present={a,p,q} definite={a} closed
    return df["p"]                                 #@ possibly p


def copies(df):
    for c in list(df.columns):
        df[c + "_copy"] = df[c]
    n = len(df.columns)                            #@row df: present={a,b,a_copy,b_copy} definite={a,a_copy} closed
    return df["a_copy"]                            #@ ok a_copy


def until(df, k):
    i = 0
    while i < k:
        df[f"w{i}"] = i
        i += 1
    n = len(df.columns)                            #@row df: present={a,w0,w1} definite={a} closed
    return df["w0"]                                #@ possibly w0


if __name__ == "__main__":
    add_named(pd.DataFrame({"a": [1]}))
    add_named(pd.DataFrame({"a": [], "x": []}))
    add_lags(pd.DataFrame({"a": [1, 2, 3, 4]}))
    add_lags(pd.DataFrame({"a": [5], "b": ["u"]}))
    for names in (["p"], [], ["p", "q"]):
        try:
            add_from(pd.DataFrame({"a": [1]}), names)
        except KeyError:
            pass
    copies(pd.DataFrame({"a": [1], "b": [2]}))
    copies(pd.DataFrame({"a": [3]}))
    for k in (0, 2):
        try:
            until(pd.DataFrame({"a": [1]}), k)
        except KeyError:
            pass
