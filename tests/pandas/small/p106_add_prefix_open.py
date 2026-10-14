# add_prefix / add_suffix: every column is renamed, so an unprefixed name is gone even on an open frame
import io

import pandas as pd

CSV = "qty,price,memo\n1,2.0,x\n3,4.0,y\n"

kept = []  # caught failures stay alive: the oracle tells exceptions apart by id()

raw = pd.read_csv(io.StringIO(CSV))
p = raw.add_prefix("in_")
p["in_qty"]                                      #@ input in_qty
# (a row lists the absent keys it saw; qty was never seen: the access below checks it)
#@todo row p: absent={qty}
#@row p: definite={in_qty} open
try:
    p["qty"]                                     #@ missing qty
except KeyError as e:
    kept.append(e)
# (dropping qty makes the read of in_qty fail)
#@schema read_csv@L10: always={qty} only={qty}

c = pd.DataFrame({"a": [1], "b": [2]}).add_suffix("_r")
#@row c: present={a_r,b_r} definite={a_r,b_r} absent={a,b} closed
c["b_r"]                                         #@ ok b_r
try:
    c["a"]                                       #@ missing a
except KeyError as e:
    kept.append(e)
