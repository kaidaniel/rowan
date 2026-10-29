"""More DataFrame shapes: column assignment, drop/rename/merge/concat/groupby, iterrows,
to_dict records, filtering, apply on rows."""
import pandas as pd

df = pd.read_csv("orders.csv")
df["total"] = df["qty"] * df["price"]            #@ input qty ; input price
df = df.rename(columns={"cust": "customer"})
df["customer"]                                   #@ input customer
df["cust"]                                       #@ missing cust
df = df.drop(columns=["price"])
df["price"]                                      #@ missing price
df["total"]                                      #@ ok total
sub = df[["total", "customer"]]
sub["total"]                                     #@ ok total
sub["qty"]                                       #@ missing qty
agg = df.groupby("customer").agg(spent=("total", "sum"))
agg["spent"]                                     #@ ok spent
agg["customer"]                                  #@ missing customer
agg2 = agg.reset_index()
agg2["customer"]                                 #@ ok customer
both = pd.concat([agg2, agg2.assign(year=1)])
both["year"]                                     #@ ok year
both["spent"]                                    #@ ok spent
both["other"]                                    #@ missing other
m = df.merge(agg2, on="customer")
# (was `ok spent`: df is an open read_csv frame; a `spent` column of the input would make the
# merge give spent_x / spent_y instead: there on the paths where the input lacks it)
m["spent"]                                       #@ possibly spent
m["total"]                                       #@ ok total
# the frame came from read_csv: unknown columns depend on the input
m["zzz"]                                         #@ input zzz
for _, row in df.iterrows():
    row["total"]                                 #@ ok total
    row["price"]                                 #@ missing price
    row["region"]                                #@ input region
recs = df.to_dict("records")
for r in recs:
    r["customer"]                                #@ ok customer
    r["nope"]                                    #@ input nope


def flag(row):
    return {"flag": row["total"] > 10, "big": row["qty"] > 5}   #@ ok total ; ok qty


flags = df.apply(flag, axis=1)
# (was `ok flag` / `missing small`, then `none`: a Series of dicts, one per row, unless
# result_type="expand", labelled 0..n-1; a frame of no rows gives its empty copy, without flag)
#@todo missing flag ; missing small (labels of a read_csv frame are 0..n-1)
flags["flag"]                                    #@ possibly flag
flags["small"]                                   #@ possibly small
wide = pd.DataFrame([{"a": 1, "b": 2}, {"a": 3, "c": 4}])
wide["c"]                                        #@ ok c
wide["d"]                                        #@ missing d
filtered = wide[wide["a"] > 1]
filtered["b"]                                    #@ ok b
filtered["d"]                                    #@ missing d
wide2 = wide.copy()
wide2["e"] = 1
wide["e"]                                        #@ missing e
wide2["e"]                                       #@ ok e
#@schema read_csv@L5: always={qty,price,cust,region}
