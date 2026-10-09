# groupby on several keys: all leave the columns; reset_index / as_index=False bring every key back
import pandas as pd

M1 = pd.DataFrame({"y": [2024, 2024, 2025], "q": [1, 2, 1], "rev": [10, 20, 30]})
M2 = pd.DataFrame({"y": [2026], "q": [3], "rev": [5], "cost": [2]})


def by_period(df):
    g = df.groupby(["y", "q"]).sum()
    #@row g: present={rev,cost} definite={rev} absent={y,q} closed
    r = g.reset_index()
    #@row r: present={y,q,rev,cost} definite={y,q,rev} closed
    r["q"]                                               #@ ok q
    flat = df.groupby(["y", "q"], as_index=False)["rev"].sum()
    #@row flat: present={y,q,rev} definite={y,q,rev} closed
    one = df.groupby(["y", "q"])["rev"].max().reset_index()
    #@row one: present={y,q,rev} definite={y,q,rev} absent={cost} closed
    return g


def year(g):
    return g["y"]                                        #@ missing y


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (M1, M2):
        try:
            year(by_period(df))
        except KeyError as e:
            kept.append(e)
