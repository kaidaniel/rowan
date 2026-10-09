# iterrows: a row's keys are the frame's columns; row["k"] verdicts per call, per branch, on read_csv input
import io

import pandas as pd

CSV = "id,qty,note\n1,2,a\n2,5,b\n"


def quantities(df):
    out = []
    for _, row in df.iterrows():
        q = row["qty"]                                   #@ ok qty
        #@row row: present={qty,price,region} definite={qty} closed
        out.append(q)
    return out


def price_of(df):
    for _, row in df.iterrows():
        return row["price"]                              #@ ok price ; missing price
    return None


def tagged(df, flag):
    if flag:
        df = df.assign(tag="t")
    for _, row in df.iterrows():
        return row["tag"]                                #@ possibly tag
    return None


raw = pd.read_csv(io.StringIO(CSV))
notes = []
for _, r in raw.iterrows():
    n = r["note"]                                        #@ input note
    #@row r: definite={note} open
    notes.append(n)
#@schema read_csv@L32: always={note} only={note}

if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    a = pd.DataFrame({"qty": [1, 2], "price": [3.0, 4.0]})
    b = pd.DataFrame({"qty": [7], "region": ["n"]})
    c = pd.DataFrame({"qty": [1, 2], "price": [1.0, 2.0], "region": ["s", "e"]})
    quantities(a)
    quantities(b)
    quantities(c)
    price_of(a)
    price_of(c)
    try:
        price_of(b)
    except KeyError as e:
        caught.append(e)
    tagged(a, True)
    tagged(c, True)
    try:
        tagged(b, False)
    except KeyError as e:
        caught.append(e)
