# input schemas through a merge: each read_csv must hold the merge key and the columns read after the merge
import io

import pandas as pd

ORDERS = "order,cust,amount,note\n1,10,5.0,x\n2,11,6.0,y\n3,10,1.0,z\n"
CUSTOMERS = "cust,name,region\n10,a,e\n11,b,w\n"

orders = pd.read_csv(io.StringIO(ORDERS))
customers = pd.read_csv(io.StringIO(CUSTOMERS))
m = orders.merge(customers, on="cust", how="left")      #@ input cust
#@row m: definite={cust} open
m["amount"]                                              #@ input amount
by_region = m.groupby("region")["amount"].sum().reset_index()   #@ input region
#@row by_region: present={region,amount} definite={region,amount} closed
by_region["amount"]                                      #@ ok amount
#@schema read_csv@L9: always={cust,amount} only={cust,amount}
#@schema read_csv@L10: always={cust,region} only={cust,region}
