# rolling / expanding / ewm(...).mean() on a frame: the same columns; rolling(on=c) keeps c; groupby().rolling drops the key
import pandas as pd

W1 = pd.DataFrame({"t": [1, 2, 3, 4], "x": [1.0, 2.0, 3.0, 4.0], "y": [0.0, 1.0, 0.0, 1.0]})
W2 = pd.DataFrame({"t": [1, 2], "x": [5.0, 6.0], "y": [1.0, 1.0], "z": [2.0, 3.0]})


def smooth(df):
    r = df.rolling(2).mean()
    #@row r: present={t,x,y,z} definite={t,x,y} closed
    e = df.expanding().sum()
    #@row e: present={t,x,y,z} definite={t,x,y} closed
    w = df[["x", "y"]].ewm(span=3).mean()
    #@row w: present={x,y} definite={x,y} absent={t,z} closed
    o = df.rolling(2, on="t").sum()
    #@row o: present={t,x,y,z} definite={t,x,y} closed
    o["t"]                                               #@ ok t
    return r


def z(r):
    return r["z"]                                        #@ possibly z


def grouped(df):
    g = df.assign(k=[1] * len(df)).groupby("k").rolling(2).sum()
    #@row g: present={t,x,y,z} definite={t,x,y} absent={k} closed
    return g["k"]                                        #@ missing k


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    z(smooth(W2))
    try:
        z(smooth(W1))
    except KeyError as e:
        kept.append(e)
    for df in (W1, W2):
        try:
            grouped(df)
        except KeyError as e:
            kept.append(e)
