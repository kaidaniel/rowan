# transform: same rows, the selected (or all non-key) columns; a Series transform stored back as a column
import pandas as pd

T1 = pd.DataFrame({"k": ["a", "a", "b"], "x": [1, 2, 3], "y": [4, 5, 6]})
T2 = pd.DataFrame({"k": ["c"], "x": [7], "y": [8], "w": [9]})


def shares(df):
    t = df.groupby("k").transform("sum")
    #@row t: present={x,y,w} definite={x,y} absent={k} closed
    sel = df.groupby("k")[["x"]].transform("mean")
    #@row sel: present={x} definite={x} absent={k,y} closed
    df = df.copy()
    df["x_tot"] = df.groupby("k")["x"].transform("sum")
    #@row df: present={k,x,y,w,x_tot} definite={k,x,y,x_tot} closed
    df["x_tot"]                                          #@ ok x_tot
    return t


def w(t):
    return t["w"]                                        #@ possibly w


def key(t):
    return t["k"]                                        #@ missing k


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    w(shares(T2))
    try:
        w(shares(T1))
    except KeyError as e:
        kept.append(e)
    for df in (T1, T2):
        try:
            key(shares(df))
        except KeyError as e:
            kept.append(e)
