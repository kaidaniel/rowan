import pandas as pd

df = pd.read_csv("sales.csv")
g = df.groupby("region").agg(total=("amount", "sum"), n=("order_id", "count"))
g["total"]                                      #@ ok total
g["amount"]                                     #@ missing amount
g2 = g.reset_index()
g2["region"]                                    #@ ok region
both = pd.concat([df.assign(year=2024), df.assign(year=2025)])
both["year"]                                    #@ ok year
both["amount"]                                  #@ ok amount
s = df.groupby("region")["amount"].sum()
cols = df.groupby("region").sum()
cols["region"]                                  #@ missing region
cols2 = cols.reset_index()
cols2["region"]                                 #@ ok region
df2 = df.set_index("order_id")
df2["order_id"]                                 #@ missing order_id
df2.reset_index()["order_id"]                   #@ ok order_id
df3 = df.add_prefix("s_")
df3["s_amount"]                                 #@ ok s_amount
df3["amount"]                                   #@ missing amount
df.loc[df["amount"] > 0, "flag"] = 1
df["flag"]                                      #@ ok flag
#@schema read_csv@L3: always={region,amount,order_id}
