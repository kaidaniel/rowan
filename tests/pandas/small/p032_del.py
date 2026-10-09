# del df[col]: removes in place; deleting twice raises KeyError; del on one branch, in a loop, on a copy (the original keeps it)
import io

import pandas as pd

CSV = "a,c,d\n1,2,3\n"


def remove(df):
    del df["b"]                                    #@ ok b
    n = len(df.columns)                            #@row df: present={a,c} definite={a} absent={b} closed
    del df["b"]                                    #@ missing b


def maybe(df, flag):
    if flag:
        del df["a"]                                #@ ok a
    n = len(df.columns)                            #@row df: present={a,b} definite={b} closed
    return df["a"]                                 #@ possibly a


def loop(df):
    for c in ["x", "y"]:
        del df[c]
    n = len(df.columns)                            #@row df: present={a,z} definite={a} absent={x,y} closed
    return df


def on_copy(df):
    c = df.copy()
    del c["a"]                                     #@ ok a
    n = len(c.columns)                             #@row c: present={b} definite={b} absent={a} closed
    #@row df: present={a,b} definite={a,b} closed
    return df["a"]                                 #@ ok a


raw = pd.read_csv(io.StringIO(CSV))
del raw["c"]                                       #@ input c
#@row raw: absent={c} open
raw["a"]                                           #@ input a
#@schema read_csv@L37: always={a,c} only={a,c}

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1], "b": [2]}), pd.DataFrame({"a": [1], "b": [2], "c": [3]})):
        try:
            remove(f)
        except KeyError:
            pass
    for flag in (True, False):
        try:
            maybe(pd.DataFrame({"a": [1], "b": [2]}), flag)
        except KeyError:
            pass
    loop(pd.DataFrame({"a": [1], "x": [2], "y": [3]}))
    loop(pd.DataFrame({"a": [1], "x": [2], "y": [3], "z": [4]}))
    on_copy(pd.DataFrame({"a": [1], "b": [2]}))
    on_copy(pd.DataFrame({"a": [], "b": []}))
