# DataFrame.drop: columns=, labels with axis=1 / axis="columns", errors="ignore", inplace=True; dropping index labels keeps the columns
import io

import pandas as pd

CSV = "id,name,tmp,extra\n1,x,0,q\n2,y,1,r\n"


def by_columns(df):
    out = df.drop(columns=["b"])
    #@row out: present={a,c} definite={a} absent={b} closed
    #@row df: present={a,b,c} definite={a,b} closed
    out["b"]                                       #@ missing b


def by_axis(df):
    out = df.drop(["b", "c"], axis=1)
    #@row out: present={a} definite={a} absent={b,c} closed
    out2 = df.drop("a", axis="columns")
    #@row out2: present={b,c} definite={b,c} absent={a} closed
    return out2["c"]                               #@ ok c


def tolerant(df):
    out = df.drop(columns=["b", "zz"], errors="ignore")
    #@row out: present={a,c} definite={a} absent={b,zz} closed
    return out


def in_place(df):
    res = df.drop(columns="b", inplace=True)
    #@value res: exactly={None}
    #@row df: present={a,c} definite={a} absent={b} closed
    return df


def rows(df):
    out = df.drop(index=[0])
    #@row out: present={a,b} definite={a,b} closed
    out2 = df.drop(0)
    #@row out2: present={a,b} definite={a,b} closed
    return out2["b"]                               #@ ok b


raw = pd.read_csv(io.StringIO(CSV))
raw["tmp"].sum()                                   #@ input tmp
clean = raw.drop(columns=["tmp"])
#@row clean: absent={tmp} open
clean["name"]                                      #@ input name
#@schema read_csv@L45: always={tmp,name} only={tmp,name}

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1], "b": [2]}), pd.DataFrame({"a": [1], "b": [2], "c": [3]})):
        try:
            by_columns(f)
        except KeyError:
            pass
    by_axis(pd.DataFrame({"a": [1], "b": [2], "c": [3]}))
    by_axis(pd.DataFrame({"a": [], "b": [], "c": []}))
    tolerant(pd.DataFrame({"a": [1], "b": [2]}))
    tolerant(pd.DataFrame({"a": [1], "c": [2]}))
    tolerant(pd.DataFrame({"a": [1], "b": [2], "c": [3]}))
    in_place(pd.DataFrame({"a": [1], "b": [2]}))
    in_place(pd.DataFrame({"a": [1], "b": [2], "c": [3]}))
    rows(pd.DataFrame({"a": [1, 2], "b": [3, 4]}))
    rows(pd.DataFrame({"a": [1], "b": [3]}))
