# to_dict("records") of a read_csv frame: record keys are the input's columns; what the record accesses demand of the input
import io

import pandas as pd

CSV = "sku,qty,price,warehouse\nA,1,2.5,w1\nB,3,1.0,w2\n"


def line_total(r):
    return r["qty"] * r["price"]                         #@ input qty ; input price


def short_price(recs):
    return recs[0]["price"]                              #@ missing price


inv = pd.read_csv(io.StringIO(CSV))
recs = inv.to_dict("records")
totals = [line_total(r) for r in recs]
first = recs[0]
code = first["sku"]                                      #@ input sku
#@row first: definite={sku} open
short = inv[["sku", "qty"]].to_dict("records")
s0 = short[0]                                            #@row s0: present={sku,qty} definite={sku,qty} absent={price} closed
#@schema read_csv@L17: always={sku,qty,price} only={sku,qty,price}

if __name__ == "__main__":
    caught = []  # keeps each raised exception alive
    try:
        short_price(short)
    except KeyError as e:
        caught.append(e)
