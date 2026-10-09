# pivot_table: values/index/columns/aggfunc; the columns are the data values of columns=, margins=True adds "All" (or margins_name); no columns= keeps the value names
import io

import pandas as pd

CSV = "day,kind,amount,memo\nmon,in,3,x\nmon,out,1,y\ntue,in,5,z\n"


def by_kind(df):
    t = df.pivot_table(values="amount", index="day", columns="kind", aggfunc="sum")
    #@row t: present={in,out,fee} definite={in,out} absent={day,kind,amount,All} closed
    m = df.pivot_table(values="amount", index="day", columns="kind", aggfunc="sum", margins=True)
    #@row m: present={in,out,fee,All} definite={in,out,All} absent={day,kind,amount} closed
    m2 = pd.pivot_table(df, values="amount", index="day", columns="kind", aggfunc="max", margins=True, margins_name="Total")
    #@row m2: present={in,out,fee,Total} definite={in,out,Total} absent={All} closed
    m["All"]                                       #@ ok All
    t["All"]                                       #@ missing All


def per_day(df):
    t = df.pivot_table(index="day", values=["amount", "qty"], aggfunc="mean")
    #@row t: present={amount,qty} definite={amount,qty} absent={day,kind} closed
    flat = t.reset_index()
    #@row flat: present={day,amount,qty} definite={day,amount,qty} absent={kind} closed
    t2 = pd.pivot_table(df, index="day", values=["amount", "qty"], aggfunc={"amount": "sum", "qty": "max"})
    #@row t2: present={amount,qty} definite={amount,qty} absent={day} closed
    return flat


raw = pd.read_csv(io.StringIO(CSV))
pt = raw.pivot_table(values="amount", index="day", columns="kind", aggfunc="sum", margins=True)
#@row pt: definite={All} open
pt["All"]                                          #@ ok All
#@schema read_csv@L30: always={day,kind,amount} only={day,kind,amount}

if __name__ == "__main__":
    a = pd.DataFrame({"day": ["mon", "mon", "tue"], "kind": ["in", "out", "in"], "amount": [3, 1, 5], "qty": [1, 1, 2]})
    b = pd.DataFrame({"day": ["mon", "tue", "tue"], "kind": ["in", "out", "fee"], "amount": [2, 4, 1], "qty": [3, 1, 1]})
    for f in (a, b):
        try:
            by_kind(f)
        except KeyError:
            pass
        per_day(f)
