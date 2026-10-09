# pd.crosstab: the columns are the values of the second factor, the index the first; margins adds "All", rownames/colnames name the axes, values/aggfunc
import io

import pandas as pd

CSV = "shop,kind,qty,memo\nn,in,1,x\ns,out,2,y\n"


def table(df):
    t = pd.crosstab(df["shop"], df["kind"])        #@ ok shop ; ok kind
    #@row t: present={in,out} definite={in} absent={shop,kind,All} closed
    flat = t.reset_index()
    #@row flat: present={shop,in,out} definite={shop,in} closed
    m = pd.crosstab(df["shop"], df["kind"], margins=True)
    #@row m: present={in,out,All} definite={in,All} closed
    m["All"]                                       #@ ok All
    t["All"]                                       #@ missing All


def named(df):
    t = pd.crosstab(df["shop"], df["kind"], rownames=["store"], colnames=["k"])
    flat = t.reset_index()
    #@row flat: present={store,in,out} definite={store,in} absent={shop,k} closed
    v = pd.crosstab(df["shop"], df["kind"], values=df["qty"], aggfunc="sum")
    #@row v: present={in,out} definite={in} absent={qty} closed
    return flat


raw = pd.read_csv(io.StringIO(CSV))
ct = pd.crosstab(raw["shop"], raw["kind"], margins=True)   #@ input shop ; input kind
#@row ct: definite={All} open
ct["All"]                                          #@ ok All
#@schema read_csv@L29: always={shop,kind} only={shop,kind}

if __name__ == "__main__":
    a = pd.DataFrame({"shop": ["n", "s", "n"], "kind": ["in", "out", "out"], "qty": [1, 2, 3]})
    b = pd.DataFrame({"shop": ["n", "s"], "kind": ["in", "in"], "qty": [4, 5]})
    for f in (a, b):
        try:
            table(f)
        except KeyError:
            pass
        named(f)
