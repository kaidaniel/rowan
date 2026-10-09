# corr / cov (numeric_only) are square over the numeric columns; idxmax is a Series over the columns whose values are row labels
import pandas as pd


def relations(df):
    c = df.corr(numeric_only=True)                       #@row c: present={a,b,c} definite={a,b} absent={name} closed
    row = c.loc["a"]                                     #@row row: present={a,b,c} definite={a,b} closed
    cv = df.cov(numeric_only=True)                       #@row cv: present={a,b,c} definite={a,b} absent={name} closed
    im = df.idxmax(numeric_only=True)                    #@row im: present={a,b,c} definite={a,b} absent={name} closed
    c["b"]                                               #@ ok b
    return c["name"]                                     #@ missing name


grid = pd.DataFrame({"x": [1, 5], "y": [3, 2]})
best = grid.idxmax()                                     #@row best: present={x,y} definite={x,y} closed
# (which label holds the maximum needs the values compared: rowan knows the candidates)
#@todo value bx: exactly={1} ; best_col: exactly={"y"}
bx = best["x"]                                           #@value bx: includes={1}
best_col = grid.idxmax(axis=1)[0]                        #@value best_col: includes={"y"}

if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    f1 = pd.DataFrame({"a": [1, 2, 3], "b": [3.0, 1.0, 2.0], "name": ["p", "q", "r"]})
    f2 = pd.DataFrame({"a": [4, 5], "b": [1, 0], "c": [2.0, 9.0]})
    for frame in (f1, f2):
        try:
            relations(frame)
        except KeyError as e:
            caught.append(e)
