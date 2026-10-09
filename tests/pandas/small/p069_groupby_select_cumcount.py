# groupby(k)[[cols]] narrows to those columns; [col].agg(...).reset_index() gives {k, col}; cumcount is a Series
import pandas as pd

G1 = pd.DataFrame({"k": ["a", "b", "a"], "a": [1, 2, 3], "b": [4, 5, 6], "c": [7, 8, 9]})
G2 = pd.DataFrame({"k": ["z"], "a": [1], "b": [2], "c": [3], "d": [4]})


def pick(df):
    s = df.groupby("k")[["a", "b"]].sum()                #@ ok a ; ok b
    #@row s: present={a,b} definite={a,b} absent={c,d,k} closed
    one = df.groupby("k")["c"].max().reset_index()       #@ ok c
    #@row one: present={k,c} definite={k,c} closed
    df = df.assign(seq=df.groupby("k").cumcount())
    #@row df: present={k,a,b,c,d,seq} definite={k,a,b,c,seq} closed
    return s


def c_of(s):
    return s["c"]                                        #@ missing c


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (G1, G2):
        try:
            c_of(pick(df))
        except KeyError as e:
            kept.append(e)
