# rename(columns={...}): mapped names replace the old ones, mapping keys that are not columns are ignored, errors="raise", inplace=True, swaps, chains
import io

import pandas as pd

CSV = "Cust ID,Amt,Note\n1,2,x\n"


def basic(df):
    out = df.rename(columns={"a": "alpha", "zz": "never"})
    #@row out: present={alpha,b} definite={alpha} absent={a,zz,never} closed
    out["alpha"]                                   #@ ok alpha
    out["a"]                                       #@ missing a


def swap(df):
    out = df.rename(columns={"a": "b", "b": "a"})
    #@row out: present={a,b} definite={a,b} closed
    return out


def strict(df):
    out = df.rename(columns={"a": "x"}, errors="raise")
    #@row out: present={x,b} definite={x} absent={a} closed
    return out


def in_place(df):
    res = df.rename(columns={"b": "beta"}, inplace=True)
    #@value res: exactly={None}
    #@row df: present={a,beta} definite={a,beta} absent={b} closed
    return df


def chain(df):
    out = df.rename(columns={"a": "b"}).rename(columns={"b": "c"})
    #@row out: present={c,x} definite={c} absent={a,b} closed
    return out


raw = pd.read_csv(io.StringIO(CSV))
ren = raw.rename(columns={"Cust ID": "cust_id", "Amt": "amount"})
#@row ren: absent={"Cust ID",Amt} open
ren["cust_id"]                                     #@ input cust_id
try:
    ren["Amt"]                                     #@ missing Amt
except KeyError:
    pass

if __name__ == "__main__":
    for f in (pd.DataFrame({"a": [1]}), pd.DataFrame({"a": [1], "b": [2]})):
        try:
            basic(f)
        except KeyError:
            pass
    swap(pd.DataFrame({"a": [1], "b": [2]}))
    swap(pd.DataFrame({"a": [], "b": []}))
    for f in (pd.DataFrame({"a": [1], "b": [2]}), pd.DataFrame({"a": [1]}), pd.DataFrame({"b": [1]})):
        try:
            strict(f)
        except KeyError:
            pass
    in_place(pd.DataFrame({"a": [1], "b": [2]}))
    in_place(pd.DataFrame({"a": [], "b": []}))
    chain(pd.DataFrame({"a": [1]}))
    chain(pd.DataFrame({"a": [1], "x": [2]}))
