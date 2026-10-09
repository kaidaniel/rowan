# a name rebound on one branch only (df = df.drop(columns=["x"])): the joined row after the if, the caller's frame untouched
import pandas as pd

F = pd.DataFrame({"k": [1, 2], "x": [3, 4], "y": [5, 6]})


def trim(df, cut):
    if cut:
        df = df.drop(columns=["x"])
        #@row df: present={k,y} definite={k,y} absent={x} closed
    n = len(df.columns)                          #@row df: present={k,x,y} definite={k,y} closed
    #@value n: exactly={2,3}
    df["y"]                                      #@ ok y
    return df


def read_x(df):
    return df["x"]                               #@ possibly x


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    for cut in (True, False):
        out = trim(F, cut)
        try:
            read_x(out)
        except KeyError as e:
            kept.append(e)
    trim(F.head(0), True)
    F["x"]                                       #@ ok x
    #@row F: present={k,x,y} definite={k,x,y} closed
