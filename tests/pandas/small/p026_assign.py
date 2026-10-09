# DataFrame.assign: new columns, callables (also reading an earlier keyword), overwriting, chaining; the receiver keeps its columns
import io

import pandas as pd

CSV = "qty,price,region\n2,3.5,n\n1,10.0,s\n"


def double(d):
    return d["a"] * 2                                          #@ ok a


def a2_twice(d):
    return d["a2"] * 2                                         #@ ok a2


def priced(df):
    out = df.assign(total=df["qty"] * df["price"], tax=0.2)    #@ ok qty ; ok price
    #@row out: present={qty,price,region,total,tax} definite={qty,price,total,tax} closed
    #@row df: present={qty,price,region} definite={qty,price} absent={total,tax} closed
    out["total"]                                               #@ ok total
    return out


def chained(df):
    out = df.assign(a2=double, a4=a2_twice)
    #@row out: present={a,b,a2,a4} definite={a,a2,a4} closed
    again = out.assign(a=0, b=1)
    #@row again: present={a,b,a2,a4} definite={a,b,a2,a4} closed
    return again


def receiver_unchanged(df):
    df.assign(extra=1)
    df["extra"]                                                #@ missing extra


raw = pd.read_csv(io.StringIO(CSV))
withtot = raw.assign(total=raw["qty"] * raw["price"])         #@ input qty ; input price
#@row withtot: definite={qty,price,total} open
withtot["region"]                                              #@ input region
try:
    raw["total"]                                               #@ missing total
except KeyError:
    pass
#@schema read_csv@L38: always={qty,price,region} only={qty,price,region}

if __name__ == "__main__":
    priced(pd.DataFrame({"qty": [1, 2], "price": [2.0, 3.0]}))
    priced(pd.DataFrame({"qty": [5], "price": [1.0], "region": ["n"]}))
    chained(pd.DataFrame({"a": [1, 2]}))
    chained(pd.DataFrame({"a": [3], "b": ["x"]}))
    chained(pd.DataFrame({"a": [], "b": []}))
    for f in (pd.DataFrame({"a": [1]}), pd.DataFrame({"qty": [1], "price": [2.0]})):
        try:
            receiver_unchanged(f)
        except KeyError:
            pass
