# filter keeps whole rows (every column); apply gets each group without its key (pandas 3) and builds the result
import pandas as pd

F1 = pd.DataFrame({"k": ["a", "a", "b"], "v": [1, 2, 3]})
F2 = pd.DataFrame({"k": ["c", "c"], "v": [4, 5], "tag": ["p", "q"]})


def big(df):
    f = df.groupby("k").filter(lambda g: len(g) > 1)
    #@row f: present={k,v,tag} definite={k,v} closed
    f["k"]                                               #@ ok k
    return f


def top(g):
    # (the row directive was on its own line under `def`, which checks the `def` statement)
    return g.nlargest(1, "v")
    #@row g: present={v,tag} definite={v} absent={k} closed


def spread(g):
    return pd.Series({"lo": g["v"].min(), "hi": g["v"].max()})     #@ ok v


def summarise(df):
    t = df.groupby("k").apply(top)
    #@row t: present={v,tag} definite={v} absent={k} closed
    s = df.groupby("k").apply(spread)
    #@row s: present={lo,hi} definite={lo,hi} closed
    s["hi"]                                              #@ ok hi
    return s.reset_index()


def first_key(g):
    return g["k"].iloc[0]                                #@ missing k


def key_in_group(df):
    return df.groupby("k").apply(first_key)


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (F1, F2):
        big(df)
        r = summarise(df)
        #@row r: present={k,lo,hi} definite={k,lo,hi} closed
        try:
            key_in_group(df)
        except KeyError as e:
            kept.append(e)
