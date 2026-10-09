# read_csv input used only through rows (apply axis=1, itertuples, records): the schema the row accesses demand
import io

import pandas as pd

CSV = "id,qty,price,region,note\n1,2,3.0,n,a\n2,0,1.5,s,b\n"


def amount(row):
    return row["qty"] * row["price"]                     #@ input qty ; input price


df = pd.read_csv(io.StringIO(CSV))
df["amount"] = df.apply(amount, axis=1)
#@row df: definite={amount} open
regions = []
for t in df.itertuples(index=False):
    regions.append(t.region)                             #@ input region
    # (absent={Index} needs the file: an input column named Index would be a field)
    #@todo row t: absent={Index}
    #@row t: definite={region,amount} open
ids = []
for rec in df.to_dict("records"):
    if rec["amount"] > 0:                                #@ ok amount
        ids.append(rec["id"])                            #@ input id
#@schema read_csv@L13: always={qty,price,region} sometimes={id} only={qty,price,region,id}
