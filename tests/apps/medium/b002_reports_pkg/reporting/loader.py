"""Input data: a sales CSV (csv.DictReader over a StringIO) and a product catalogue (JSON)."""
import csv
import io
import json

SALES_CSV = """date,region,rep,sku,qty,price
2024-03-01,north,ann,A1,3,9.50
2024-03-02,south,bob,B2,1,120.00
2024-03-02,north,cat,A1,5,9.50
2024-03-05,east,ann,C3,2,15.25
2024-03-07,south,bob,A1,0,9.50
2024-03-09,north,ann,B2,2,110.00
"""

NOTES_CSV = """date,region,rep,sku,qty,price,note
2024-04-02,west,dan,A1,4,9.00,rush
2024-04-03,west,eve,C3,1,15.00,
"""

PRODUCTS_JSON = """[
  {"sku": "A1", "name": "Widget", "category": "tools"},
  {"sku": "B2", "name": "Gadget", "category": "electronics"},
  {"sku": "C3", "name": "Gizmo"}
]"""


def parse_sale(row):
    rec = {
        "date": row["date"],
        "region": row["region"],
        "rep": row["rep"],
        "sku": row["sku"],
        "qty": int(row["qty"]),
        "price": float(row["price"]),
    }
    if row.get("note"):
        rec["note"] = row["note"]
    return rec


def load_sales(text=SALES_CSV, min_qty=0):
    out = []
    for row in csv.DictReader(io.StringIO(text)):
        rec = parse_sale(row)
        if rec["qty"] >= min_qty:
            out.append(rec)
    return out


def product_entry(p):
    return p["sku"], {"name": p["name"], "category": p.get("category", "misc")}


def load_products(text=PRODUCTS_JSON):
    return dict(product_entry(p) for p in json.loads(text))
