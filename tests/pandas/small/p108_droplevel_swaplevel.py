# MultiIndex columns from agg({col: [funcs]}): droplevel keeps one level, swaplevel puts the funcs on top
import pandas as pd

D1 = pd.DataFrame({"k": ["a", "a", "b"], "x": [1, 2, 3], "y": [4, 5, 6]})
D2 = pd.DataFrame({"k": ["c"], "x": [7], "y": [8], "z": [9]})


def levels(df):
    g = df.groupby("k").agg({"x": ["sum", "mean"], "y": ["sum"]})
    #@row g: present={("x","sum"),("x","mean"),("y","sum")} definite={("x","sum"),("x","mean"),("y","sum")} closed
    top = g.droplevel(1, axis=1)
    #@row top: present={x,y} definite={x,y} closed
    low = g.droplevel(0, axis=1)
    #@row low: present={sum,mean} definite={sum,mean} closed
    sw = g.swaplevel(axis=1)
    sums = sw["sum"]                             #@ ok sum
    #@row sums: present={x,y} definite={x,y} closed
    means = sw["mean"]                           #@ ok mean
    #@row means: present={x} definite={x} absent={y} closed
    return sw


def top_x(sw):
    return sw["x"]                               #@ missing x


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for f in (D1, D2):
        try:
            top_x(levels(f))
        except KeyError as e:
            kept.append(e)
