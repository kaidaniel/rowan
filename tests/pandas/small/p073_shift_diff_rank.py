# shift / diff / cumsum / pct_change / rank on a frame: the same columns
import pandas as pd

P1 = pd.DataFrame({"a": [1.0, 2.0, 4.0], "b": [3.0, 3.0, 6.0]})
P2 = pd.DataFrame({"a": [5.0, 6.0], "b": [1.0, 2.0], "c": [9.0, 8.0]})


def changes(df):
    s = df.shift(1)
    #@row s: present={a,b,c} definite={a,b} closed
    d = df.diff()
    #@row d: present={a,b,c} definite={a,b} closed
    cs = df.cumsum()
    #@row cs: present={a,b,c} definite={a,b} closed
    p = df.pct_change().add_suffix("_pct")
    #@row p: present={a_pct,b_pct,c_pct} definite={a_pct,b_pct} absent={a,b,c} closed
    r = df.rank(ascending=False)
    #@row r: present={a,b,c} definite={a,b} closed
    r["b"]                                               #@ ok b
    return p


def plain(p):
    return p["a"]                                        #@ missing a


def c_pct(p):
    return p["c_pct"]                                    #@ possibly c_pct


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (P1, P2):
        try:
            plain(changes(df))
        except KeyError as e:
            kept.append(e)
    c_pct(changes(P2))
    try:
        c_pct(changes(P1))
    except KeyError as e:
        kept.append(e)
