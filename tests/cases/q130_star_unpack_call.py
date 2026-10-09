"""Hypothesis: `f(*row)` with a known tuple binds positionally, and `zip(*rows)` transposes
known rows.

Expectations were written from the program text before running the engine."""


def make(sku, qty, note="-"):
    return {"sku": sku, "qty": qty, "note": note}


rows = [("a", 1), ("b", 2, "x")]
r0 = make(*rows[0])                       #@row r0: present={sku,qty,note} definite={sku,qty,note} closed
r1 = make(*rows[1])                       #@row r1: present={sku,qty,note} definite={sku,qty,note} closed
cols = list(zip(*[("a", 1), ("b", 2)]))
by_col = {"skus": cols[0], "qtys": cols[1]}  #@row by_col: present={skus,qtys} definite={skus,qtys} closed
r0["extra"]                               #@ missing extra
