# What a function needs of a parameter it filters or selects from: a boolean mask
# (`raw[raw["a"].notna()]`, a mask in a variable, `.loc[mask, cols]`) or a list of columns
# keeps the parameter's columns, so a column read after it is a column of the parameter
# (`raw["qty"]`), not of one of the columns the mask was computed from
# (`raw["order_id"]["qty"]`, as rowan said). A dict read under a key computed from data is
# some item of it, not the item read just before. A parameter annotated as a frame has its
# columns' needs listed too (rowan listed none). Expectations written from the program text
# and pandas' documented behaviour; the functions get frames (a dict) with extra columns.
import pandas as pd


def keep_valid(raw):
    df = raw[raw["order_id"].notna() & raw["customer_id"].notna()].copy()
    df = df[df["qty"] > 0]
    return df["price"] * df["qty"]
#@schema param keep_valid.raw: always={order_id,customer_id,qty,price} only={order_id,customer_id,qty,price}


def pick(raw):
    m = raw["status"] != "cancelled"
    sub = raw.loc[m, ["sku", "qty"]]
    return sub["qty"].sum()
#@schema param pick.raw: always={status,sku,qty} only={status,sku,qty}


def columns(raw):
    part = raw[["a", "b"]]
    return part["b"]
#@schema param columns.raw: always={a,b} only={a,b}


def maybe(raw):
    df = raw[raw["x"] > 0]
    if "opt" in df.columns:
        return df["opt"]
    return df["x"]
#@schema param maybe.raw: always={x}  (opt was written sometimes: a read after a test that the column is there is not needed, as for dicts; raw.columns is)


def typed(raw: pd.DataFrame):
    df = raw[raw["x"] > 0]
    return df[["a", "b"]]
#@schema param typed.raw: always={x,a,b} only={x,a,b}


def lookup(d, k):
    first = d["a"]
    return first, d[k]["b"]
#@schema param lookup.d: always={a}
#@schema param lookup.d['a']: sometimes={b}  (first written none: k may be "a")


frame = pd.DataFrame({"order_id": [1, 2], "customer_id": [5, None], "qty": [1, 0], "price": [2.0, 3.0],
                      "status": ["ok", "cancelled"], "sku": ["s", "t"], "a": [1, 2], "b": [3, 4], "x": [1, -1], "opt": [7, 8], "z": [0, 0]})
v = keep_valid(frame)
p = pick(frame)
c = columns(frame)
w = maybe(frame)
t = typed(frame)
f, g = lookup({"a": {"b": 1}, "c": {"b": 2}, "e": {}}, "c")
