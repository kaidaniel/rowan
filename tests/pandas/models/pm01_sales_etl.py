# Sales ETL: orders + customers + products CSVs; cleaning, projections, merges with suffixes, derived columns, a monthly groupby report, a pivot_table
import io

import pandas as pd

ORDERS_A = """order_id,customer_id,product_id,qty,unit_price,order_date,discount,status
1,10,100,2,9.5,2024-01-03,0.1,shipped
2,11,101,1,20.0,2024-01-15,0.0,shipped
3,10,102,5,3.0,2024-02-02,0.2,cancelled
4,12,100,1,9.5,2024-02-20,,shipped
5,11,101,3,20.0,2024-03-05,0.05,returned
6,,102,1,3.0,2024-03-09,0.0,shipped
"""

# no discount column, an extra channel column
ORDERS_B = """order_id,customer_id,product_id,qty,unit_price,order_date,status,channel
7,12,101,2,20.0,2024-04-01,shipped,web
8,10,100,4,9.5,2024-04-11,SHIPPED,store
9,12,102,10,3.0,2024-05-30,shipped,web
"""

# discounts all zero, an unknown product (left merge leaves NaN)
ORDERS_C = """order_id,customer_id,product_id,qty,unit_price,order_date,discount,status
10,11,103,1,50.0,2024-06-01,0,shipped
11,11,100,2,9.5,2024-06-02,0,Cancelled
12,10,101,1,20.0,2024-07-15,0,shipped
"""

CUSTOMERS_A = """customer_id,name,region,signup_date
10, Ann ,eu,2023-05-01
11,Bob,us,2023-07-12
12,Cyd,eu,2024-01-01
"""

CUSTOMERS_B = """customer_id,name,region,signup_date,segment,email
10,Ann,eu,2023-05-01,retail,a@x.org
11,Bob,us,2023-07-12,,b@x.org
12,Cyd,EU,2024-01-01,wholesale,c@x.org
"""

PRODUCTS = """product_id,name,category,list_price
100,widget,Tools,10.0
101,gadget,Toys,22.0
102,bolt,tools,3.5
"""

PRODUCTS_B = """product_id,name,category,list_price,supplier
100,widget,tools,10.0,acme
101,gadget,toys,22.0,acme
102,bolt,tools,3.5,bolts inc
104,sample,free,0.0,acme
"""


def load_orders(text):
    raw = pd.read_csv(io.StringIO(text))
    has_discount = "discount" in raw.columns
    # (first written exactly={True, False}: a test rowan cannot decide is written `unknown`)
    #@value has_discount: unknown
    return raw


def clean_orders(raw):
    df = raw[raw["order_id"].notna() & raw["customer_id"].notna()].copy()     #@ input order_id ; input customer_id
    df["customer_id"] = df["customer_id"].astype(int)                         #@ ok customer_id
    df["product_id"] = df["product_id"].astype(int)                           #@ input product_id
    df = df[df["status"].str.lower() != "cancelled"]                          #@ input status
    df["status"] = df["status"].str.lower()                                   #@ ok status
    df["order_date"] = pd.to_datetime(df["order_date"])                       #@ input order_date
    if "discount" not in df.columns:
        df["discount"] = 0.0
    df["discount"] = df["discount"].fillna(0.0)                               #@ ok discount
    df["gross"] = df["qty"] * df["unit_price"]                                #@ input qty ; input unit_price
    df["web"] = df["channel"].eq("web") if "channel" in df.columns else False  #@ ok channel
    df = df.rename(columns={"unit_price": "price"})
    #@row df: definite={order_id,customer_id,product_id,status,order_date,discount,qty,price,gross,web} absent={unit_price} open
    df = df[["order_id", "customer_id", "product_id", "qty", "price", "discount", "gross", "order_date", "status", "web"]]
    #@row df: present={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web} definite={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web} closed
    return df


def stale_column(df):
    return df["unit_price"]                                                   #@ missing unit_price


def load_customers(text):
    c = pd.read_csv(io.StringIO(text))
    c["customer_id"] = c["customer_id"].astype(int)                           #@ input customer_id
    c["name"] = c["name"].str.strip()                                         #@ input name
    c["region"] = c["region"].str.upper()                                     #@ input region
    c["signup_date"] = pd.to_datetime(c["signup_date"])                       #@ input signup_date
    cols = ["customer_id", "name", "region", "signup_date"]
    if "segment" in c.columns:
        c["segment"] = c["segment"].fillna("unknown")                         #@ ok segment
        cols.append("segment")
    c = c[cols].drop_duplicates("customer_id")
    return c  #@row c: present={customer_id,name,region,signup_date,segment} definite={customer_id,name,region,signup_date} absent={email} closed


def load_products(text):
    p = pd.read_csv(io.StringIO(text))
    p["product_id"] = p["product_id"].astype(int)                             #@ input product_id
    p["name"] = p["name"].str.title()                                         #@ input name
    p["category"] = p["category"].str.lower()                                 #@ input category
    p = p[p["list_price"] > 0]                                                #@ input list_price
    p = p[["product_id", "name", "category", "list_price"]]
    return p  #@row p: present={product_id,name,category,list_price} definite={product_id,name,category,list_price} absent={supplier} closed


def enrich(orders, customers, products):
    m = orders.merge(customers, on="customer_id", how="left", validate="many_to_one")
    #@row m: present={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web,name,region,signup_date,segment} definite={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web,name,region,signup_date} closed
    m = m.merge(products, on="product_id", how="left", suffixes=("_cust", "_prod"))
    #@row m: present={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web,name_cust,region,signup_date,segment,name_prod,category,list_price} definite={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web,name_cust,region,signup_date,name_prod,category,list_price} absent={name,name_x,name_y} closed
    unmatched = int(m["name_prod"].isna().sum())                              #@ ok name_prod
    n_cols = len(m.columns)
    #@value n_cols: exactly={16, 17}
    return m


def bare_name(m):
    return m["name"]                                                          #@ missing name


def add_features(m):
    m = m.copy()
    m["revenue"] = m["gross"] * (1 - m["discount"])                           #@ ok gross ; ok discount
    m["month"] = m["order_date"].dt.to_period("M").astype(str)                #@ ok order_date
    m["is_eu"] = m["region"].eq("EU")                                         #@ ok region
    m = m.assign(cost=m["qty"] * m["list_price"] * 0.6)                       #@ ok qty ; ok list_price
    m["margin"] = m["revenue"] - m["cost"]                                    #@ ok cost
    m["tenure_days"] = (m["order_date"] - m["signup_date"]).dt.days           #@ ok signup_date
    if m["discount"].max() > 0:
        m["promo"] = m["discount"] > 0
    return m  #@row m: present={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web,name_cust,region,signup_date,segment,name_prod,category,list_price,revenue,month,is_eu,cost,margin,tenure_days,promo} definite={order_id,customer_id,product_id,qty,price,discount,gross,order_date,status,web,name_cust,region,signup_date,name_prod,category,list_price,revenue,month,is_eu,cost,margin,tenure_days} closed


def promo_rate(m):
    return m["promo"].mean()                                                  #@ possibly promo


def segment_of_first(m):
    return m["segment"].iloc[0]                                               #@ possibly segment


def segment_revenue(m):
    if "segment" not in m.columns:
        return None
    return m.groupby("segment")["revenue"].sum()                              #@ ok segment ; ok revenue


def monthly_report(m):
    shipped = m[m["status"] == "shipped"]                                     #@ ok status
    rep = shipped.groupby("month", as_index=False).agg(orders=("order_id", "nunique"), units=("qty", "sum"), revenue=("revenue", "sum"), margin=("margin", "sum"))
    #@row rep: present={month,orders,units,revenue,margin} definite={month,orders,units,revenue,margin} closed
    rep["avg_order"] = rep["revenue"] / rep["orders"]                         #@ ok revenue ; ok orders
    rep = rep.sort_values("month").reset_index(drop=True)
    width = len(rep.columns)
    #@value width: exactly={6}
    return rep  #@row rep: present={month,orders,units,revenue,margin,avg_order} definite={month,orders,units,revenue,margin,avg_order} absent={index,qty} closed


def report_qty(rep):
    return rep["qty"]                                                         #@ missing qty


def report_lines(rep):
    out = []
    for r in rep.to_dict("records"):
        out.append(f"{r['month']}: {r['revenue']:.2f} over {r['orders']}")   #@ ok month ; ok revenue ; ok orders
    #@row r: present={month,orders,units,revenue,margin,avg_order} definite={month,orders,units,revenue,margin,avg_order} closed
    return out


def region_category_pivot(m):
    pv = m.pivot_table(index="region", columns="category", values="revenue", aggfunc="sum", fill_value=0)
    flat = pv.reset_index()
    #@row flat: definite={region} open
    flat.columns.name = None
    flat = flat.rename(columns=lambda c: c if c == "region" else f"rev_{c}")
    flat["region"]                                                            #@ ok region
    return flat  #@row flat: definite={region} open


def customer_summary(m):
    g = m.groupby(["customer_id", "name_cust"])["revenue"].sum().reset_index()  #@ ok name_cust
    #@row g: present={customer_id,name_cust,revenue} definite={customer_id,name_cust,revenue} closed
    g["share"] = g["revenue"] / g["revenue"].sum()
    top = g.nlargest(2, "revenue").rename(columns={"name_cust": "customer"})
    #@row top: present={customer_id,customer,revenue,share} definite={customer_id,customer,revenue,share} absent={name_cust} closed
    labels = []
    for _, row in top.iterrows():
        labels.append(row["customer"])                                        #@ ok customer
        #@row row: present={customer_id,customer,revenue,share} definite={customer_id,customer,revenue,share} closed
    return labels


def run(orders_text, customers_text, products_text):
    orders = clean_orders(load_orders(orders_text))
    try:
        stale_column(orders)
    except KeyError:
        pass
    customers = load_customers(customers_text)
    products = load_products(products_text)
    m = enrich(orders, customers, products)
    try:
        bare_name(m)
    except KeyError:
        pass
    m = add_features(m)
    for probe in (promo_rate, segment_of_first):
        try:
            probe(m)
        except KeyError:
            pass
    segment_revenue(m)
    rep = monthly_report(m)
    try:
        report_qty(rep)
    except KeyError:
        pass
    report_lines(rep)
    region_category_pivot(m)
    return customer_summary(m)


if __name__ == "__main__":
    run(ORDERS_A, CUSTOMERS_A, PRODUCTS)
    run(ORDERS_B, CUSTOMERS_B, PRODUCTS_B)
    run(ORDERS_C, CUSTOMERS_A, PRODUCTS)

#@schema read_csv@L56: always={order_id,customer_id,product_id,status,order_date,qty,unit_price} only={order_id,customer_id,product_id,status,order_date,qty,unit_price}
#@schema read_csv@L87: always={customer_id,name,region,signup_date} sometimes={segment} only={customer_id,name,region,signup_date}
#@schema read_csv@L101: always={product_id,name,category,list_price} only={product_id,name,category,list_price}
#@schema param clean_orders.raw: always={order_id,customer_id,product_id,status,order_date,qty,unit_price}
