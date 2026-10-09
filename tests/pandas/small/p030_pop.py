# DataFrame.pop(col): removes the column in place and returns it; popping it again (or a missing one) raises KeyError; pop on one branch
import io

import pandas as pd

CSV = "k,v,w\n1,2,3\n4,5,6\n"


def take(df):
    s = df.pop("b")                                #@ ok b
    n = len(df.columns)                            #@row df: present={a,c} definite={a} absent={b} closed
    df["b"]                                        #@ missing b


def pop_twice(df):
    df.pop("a")                                    #@ ok a
    df.pop("a")                                    #@ missing a


def maybe_pop(df, drop):
    if drop:
        df.pop("x")                                #@ ok x
    n = len(df.columns)                            #@row df: present={x,y} definite={y} closed
    return df["x"]                                 #@ possibly x


def into_other(df):
    out = pd.DataFrame({"moved": df.pop("m")})     #@ ok m
    #@row out: present={moved} definite={moved} closed
    #@row df: present={k} definite={k} absent={m} closed
    return out


raw = pd.read_csv(io.StringIO(CSV))
vals = raw.pop("v")                                #@ input v
#@row raw: absent={v} open
raw["k"]                                           #@ input k
#@schema read_csv@L34: always={v,k} only={v,k}

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1], "b": [2]}), pd.DataFrame({"a": [1], "b": [2], "c": [3]})):
        try:
            take(f)
        except KeyError:
            pass
    for f in (pd.DataFrame({"a": [1]}), pd.DataFrame({"a": [], "b": []})):
        try:
            pop_twice(f)
        except KeyError:
            pass
    for drop in (True, False):
        try:
            maybe_pop(pd.DataFrame({"x": [1], "y": [2]}), drop)
        except KeyError:
            pass
    into_other(pd.DataFrame({"k": [1], "m": [2]}))
    into_other(pd.DataFrame({"k": [], "m": []}))
