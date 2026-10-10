"""Aggregations over the sales frame."""
import pandas as pd

SALE_COLUMNS = ["date", "region", "rep", "sku", "qty", "price"]


def sales_frame(sales, products):
    df = pd.DataFrame(sales, columns=SALE_COLUMNS)
    prod = pd.DataFrame([{"sku": sku, **info} for sku, info in products.items()])
    df = df.merge(prod, on="sku", how="left")
    df["revenue"] = df["qty"] * df["price"]
    return df


def by_region(df):
    g = df.groupby("region", as_index=False).agg(orders=("sku", "count"), qty=("qty", "sum"), revenue=("revenue", "sum"))
    return g.sort_values("revenue", ascending=False)


def by_category(df):
    g = df.groupby(["category"]).agg({"qty": "sum", "revenue": "sum"}).reset_index()
    return g.sort_values(["revenue", "category"], ascending=[False, True])


def top_reps(df, n):
    g = df.groupby("rep").agg(qty=("qty", "sum"), revenue=("revenue", "sum")).reset_index()
    return g.sort_values("revenue", ascending=False).head(n).to_dict("records")


def region_records(df, regions):
    sub = df[df["region"].isin(regions)]
    return by_region(sub).to_dict("records")
