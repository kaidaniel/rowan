import pandas as pd

orders = pd.read_csv(f_orders)
cust = pd.read_csv(f_cust).rename(columns={"id": "customer_id"})

orders["total"] = orders["qty"] * orders["price"]        #@ input qty ; input price
WEIGHTS = {"EU": 1.0, "US": 1.2}


def score(row):
    return row["total"] * WEIGHTS[row["region"]]         #@ ok total ; input region ; datakey <data


orders["score"] = orders.apply(score, axis=1)
orders["name"].str.strip()
cust["name"].str.strip()
m = orders.merge(cust, on="customer_id", how="left")      #@ input customer_id
m["email"]                                                #@ input email
m["name"]                                                 #@ missing name
m["name_x"]                                               #@ ok name_x
top = m[m["score"] > 1][["customer_id", "email", "name_x"]]  #@row top: present={customer_id,email,name_x} definite={customer_id,email,name_x} closed
top["qty"]                                                #@ missing qty
cust["id"]                                                #@ missing id
#@schema read_csv@L3: always={qty,price,region,name,customer_id,email}
#@schema read_csv@L4: always={id,name,email}
