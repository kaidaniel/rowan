# groupby(k).sum() / .mean() / .count() / .nunique() on the frame: every non-key column, the key leaves
import pandas as pd

N1 = pd.DataFrame({"k": [1, 1, 2], "a": [1, 2, 3], "b": [0.5, 1.5, 2.5]})
N2 = pd.DataFrame({"k": [3], "a": [4], "b": [1.0], "c": [7]})
T1 = pd.DataFrame({"k": ["x", "x"], "name": ["p", "q"], "v": [1, None]})


def sums(df):
    s = df.groupby("k").sum()                            #@ ok k
    #@row s: present={a,b,c} definite={a,b} absent={k} closed
    m = df.groupby("k").mean()
    #@row m: present={a,b,c} definite={a,b} absent={k} closed
    return s


def counts(df):
    c = df.groupby("k").count()
    #@row c: present={a,b,c,name,v} absent={k} closed
    u = df.groupby("k").nunique()
    #@row u: present={a,b,c,name,v} absent={k} closed
    back = c.reset_index()
    #@row back: present={k,a,b,c,name,v} definite={k} closed
    return c


def key_col(s):
    return s["k"]                                        #@ missing k


def c_col(s):
    return s["c"]                                        #@ possibly c


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (N1, N2):
        try:
            key_col(sums(df))
        except KeyError as e:
            kept.append(e)
    c_col(sums(N2))
    try:
        c_col(sums(N1))
    except KeyError as e:
        kept.append(e)
    for df in (N1, N2, T1):
        counts(df)
