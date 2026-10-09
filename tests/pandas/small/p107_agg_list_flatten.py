# agg(["min", "max"]) makes (col, func) columns; "_".join flattens them into col_func names
import pandas as pd

S1 = pd.DataFrame({"k": ["a", "a", "b"], "x": [1, 2, 3]})
S2 = pd.DataFrame({"k": ["c"], "x": [4], "y": [5.0]})


def stats(df):
    g = df.groupby("k").agg(["min", "max"])
    g.columns = ["_".join(c) for c in g.columns]
    #@row g: present={x_min,x_max,y_min,y_max} definite={x_min,x_max} absent={x,k} closed
    g["x_max"]                                   #@ ok x_max
    flat = g.reset_index()
    #@row flat: present={k,x_min,x_max,y_min,y_max} definite={k,x_min,x_max} closed
    one = df.groupby("k")["x"].agg(["min", "max"])
    #@row one: present={min,max} definite={min,max} closed
    return g


def y_min(g):
    return g["y_min"]                            #@ possibly y_min


def plain(g):
    return g["x"]                                #@ missing x


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (S1, S2):
        g = stats(f)
        for fn in (y_min, plain):
            try:
                fn(g)
            except KeyError as e:
                kept.append(e)
