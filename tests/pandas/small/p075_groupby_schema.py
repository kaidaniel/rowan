# input schema through groupby / agg: the keys and the aggregated columns are required, the rest is not
import io

import pandas as pd

SALES = "region,rep,amount,units,comment\ne,r1,5.0,1,x\nw,r2,6.0,2,y\ne,r2,1.0,3,z\n"

df = pd.read_csv(io.StringIO(SALES))
df = df[df["amount"] > 0]                                #@ input amount
df["units"] = df["units"].fillna(0)                      #@ input units
g = df.groupby("region").agg(total=("amount", "sum"), units=("units", "max"))   #@ input region
#@row g: present={total,units} definite={total,units} closed
g["total"]                                               #@ ok total
reps = df.groupby(["region", "rep"], as_index=False)["units"].sum()   #@ ok region ; input rep ; ok units
#@row reps: present={region,rep,units} definite={region,rep,units} closed
reps["rep"]                                              #@ ok rep
#@schema read_csv@L8: always={amount,region,rep,units} only={amount,region,rep,units}
