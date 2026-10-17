"""A loop over the rows of a frame built from a few rows of literal data sees each row in
turn: `iterrows()` gives (label, row) with the row's str cells, `itertuples()` named tuples
of the row's values (Python scalars) and its label. Rowan gave every iteration one row
standing for all of them.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

orders = pd.DataFrame({"sku": ["A1", "B2"], "qty": [3, 7]}, index=["o1", "o2"])
labels = []
for key, row in orders.iterrows():
    labels.append(f"{key}:{row['sku']}")
first = labels[0]
#@value first: exactly={"o1:A1"}
last = labels[-1]
#@value last: exactly={"o2:B2"}
lines = []
for t in orders.itertuples():
    lines.append(f"{t.Index} {t.sku} x{t.qty}")
line0 = lines[0]
#@value line0: exactly={"o1 A1 x3"}
n = len(lines)
#@value n: exactly={2}
plain = list(orders.itertuples(index=False, name=None))
p1 = plain[1][1]
#@value p1: includes={7}
#@todo value p1: exactly={7}  (a plain tuple's items: its fields by position)
qty = None
for _, row in orders.iterrows():
    qty = row["qty"]
#@value qty: unknown
