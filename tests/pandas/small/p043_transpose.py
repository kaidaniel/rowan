# transpose / .T: the index becomes the columns (RangeIndex -> 0..n-1, set_index("name") -> the names), the old columns become the index
import pandas as pd


def flip(df):
    t = df.T
    #@row t: present={0,1} definite={0} absent={a,b} closed
    t[0]                                           #@ ok 0
    t["a"]                                         #@ missing a


def by_name(df):
    t = df.set_index("name").transpose()
    #@row t: present={x,y} definite={x} absent={name,v,w} closed
    back = t.loc["v"]
    return t["x"]                                  #@ ok x


def twice(df):
    back = df.T.T
    #@row back: present={a,b} definite={a,b} closed
    return back


wide = pd.DataFrame({"q1": [10], "q2": [20]}, index=["sales"]).T
#@row wide: present={sales} definite={sales} absent={q1,q2} closed
wide["sales"]                                      #@ ok sales

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1, 2], "b": [3, 4]}), pd.DataFrame({"a": [5]})):
        try:
            flip(f)
        except KeyError:
            pass
        twice(f.assign(b=0))
    by_name(pd.DataFrame({"name": ["x", "y"], "v": [1, 2], "w": [3, 4]}))
    by_name(pd.DataFrame({"name": ["x"], "v": [5]}))
