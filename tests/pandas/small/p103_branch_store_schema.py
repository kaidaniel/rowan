# a column stored on one branch of an open read_csv frame, then read: the file needs it only on the other path
import io

import pandas as pd

BIG = "id,qty,price\n1,200,3.0\n2,5,1.5\n"
SMALL = "id,qty,total\n1,2,6.0\n2,0,0.0\n"


def load(text):
    df = pd.read_csv(io.StringIO(text))
    if df["qty"].sum() > 100:                    #@ input qty
        df["total"] = df["qty"] * df["price"]    #@ input price
        #@row df: definite={qty,price,total} open
    t = df["total"]                              #@ input total
    #@row df: definite={qty,total} open
    return t.sum()


#@schema read_csv@L11: always={qty} sometimes={price,total} only={qty,price,total}


def built(n):
    df = pd.DataFrame({"qty": list(range(n)), "price": [1.0] * n})
    if n > 1:
        df["total"] = df["qty"] * df["price"]    #@ ok qty ; ok price
    return df                                    #@row df: present={qty,price,total} definite={qty,price} closed


def total(df):
    return df["total"]                           #@ possibly total


kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

if __name__ == "__main__":
    load(BIG)
    load(SMALL)
    for n in (3, 0, 1):
        try:
            total(built(n))
        except KeyError as e:
            kept.append(e)
