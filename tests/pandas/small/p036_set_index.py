# set_index: a key moves into the index (gone from the columns), a list moves both, drop=False keeps it, append=True, inplace=True
import io

import pandas as pd

CSV = "sku,qty,memo\na,1,x\nb,2,y\n"


def single(df):
    out = df.set_index("id")                       #@ ok id
    #@row out: present={v,w} definite={v} absent={id} closed
    out["v"]                                       #@ ok v
    out["id"]                                      #@ missing id


def pair(df):
    out = df.set_index(["id", "day"])
    #@row out: present={v} definite={v} absent={id,day} closed
    back = out.reset_index()
    #@row back: present={id,day,v} definite={id,day,v} absent={index} closed
    return back


def keep(df):
    out = df.set_index("id", drop=False)
    #@row out: present={id,day,v} definite={id,day,v} closed
    return out["id"]                               #@ ok id


def appended(df):
    out = df.set_index("id").set_index("day", append=True)
    #@row out: present={v} definite={v} absent={id,day} closed
    flat = out.reset_index()
    #@row flat: present={id,day,v} definite={id,day,v} absent={index} closed
    return flat


def in_place(df):
    res = df.set_index("id", inplace=True)
    #@value res: exactly={None}
    #@row df: present={day,v} definite={day,v} absent={id} closed
    return df


raw = pd.read_csv(io.StringIO(CSV))
raw["sku"].nunique()                               #@ input sku
ix = raw.set_index("sku")
#@row ix: absent={sku} open
ix["qty"]                                          #@ input qty
#@schema read_csv@L45: always={sku,qty} only={sku,qty}

if __name__ == "__main__":
    for f in (pd.DataFrame({"id": [1], "v": [2]}), pd.DataFrame({"id": [1, 2], "v": [2, 3], "w": [4, 5]})):
        try:
            single(f)
        except KeyError:
            pass
    for fn in (pair, keep, appended, in_place):
        fn(pd.DataFrame({"id": [1, 2], "day": ["mon", "tue"], "v": [3, 4]}))
        fn(pd.DataFrame({"id": [], "day": [], "v": []}))
