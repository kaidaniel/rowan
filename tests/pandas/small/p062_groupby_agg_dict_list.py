# agg with a dict of single functions: flat {cols}; a list (or a list in the dict): MultiIndex columns (col, func)
import pandas as pd

D1 = pd.DataFrame({"k": ["a", "b", "a"], "x": [1, 2, 3], "y": [4.0, 5.0, 6.0]})
D2 = pd.DataFrame({"k": ["c"], "x": [7], "y": [8.0], "z": [9]})


def by_dict(df):
    g = df.groupby("k").agg({"x": "sum", "y": "mean"})
    #@row g: present={x,y} definite={x,y} absent={k,z} closed
    g["y"]                                               #@ ok y
    return g


def by_list(df):
    g = df.groupby("k")[["x", "y"]].agg(["min", "max"])
    g["x"]                                               #@ ok x
    g[("y", "max")]                                      #@ ok ("y", "max")
    g.columns = [a + "_" + b for a, b in g.columns]
    #@row g: present={x_min,x_max,y_min,y_max} definite={x_min,x_max,y_min,y_max} closed
    return g


def mixed(df):
    g = df.groupby("k").agg({"x": "sum", "y": ["min", "max"]})
    g[("x", "sum")]                                      #@ ok ("x", "sum")
    return g["z"]                                        #@ missing z


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for df in (D1, D2):
        by_dict(df)
        by_list(df)
        try:
            mixed(df)
        except KeyError as e:
            kept.append(e)
