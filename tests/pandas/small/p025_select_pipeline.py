# a read_csv input narrowed by selection: accesses narrow the open frame, branches make columns possible
import io

import pandas as pd

CSV = "sku,qty,price,warehouse\na,1,2.0,w1\nb,0,3.5,w2\n"


def prepare(text):
    df = pd.read_csv(io.StringIO(text))
    #@row df: open
    df["qty"]                                    #@ input qty
    #@row df: definite={qty} open
    if df["qty"].sum() > 0:
        df["value"] = df["qty"] * df["price"]    #@ input price
    cols = df[["sku", "qty"]]                    #@ ok qty
    #@row cols: present={sku,qty} definite={sku,qty} closed
    df["value"]                                  #@ possibly value
    #@row df: definite={qty,value} open


raw = pd.read_csv(io.StringIO(CSV))
raw.sku                                          #@ input sku
view = raw.loc[raw["qty"] > 0, ["sku", "price"]] #@ input qty
#@row view: present={sku,price} definite={sku,price} closed
#@row raw: definite={sku,qty,price} open
try:
    view["qty"]                                  #@ missing qty
except KeyError:
    pass
#@schema read_csv@L10: always={qty,sku} sometimes={price} only={qty,sku,price}
#@schema read_csv@L22: always={sku,qty,price} only={sku,qty,price}

if __name__ == "__main__":
    caught = []
    for text in [CSV, "sku,qty\na,0\n", "qty,sku,price\n3,z,1.0\n4,y,2.0\n"]:
        try:
            prepare(text)
        except KeyError as e:
            caught.append(e)
