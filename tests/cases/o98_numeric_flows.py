"""Numbers that decide keys: counters, sums over known items, comparisons between computed
amounts and thresholds, integer division and modulo, formatted labels with widths.

Expectations were written from the program text before running the engine."""

PRICES = {"a": 250, "b": 1000, "c": 75}
THRESHOLDS = {"bulk": 1000, "free": 2000}


def line_total(sku, qty):
    return PRICES[sku] * qty


def quote(items):
    q = {"lines": [], "subtotal": 0, "n": len(items)}
    for sku, qty in items:
        t = line_total(sku, qty)
        q["lines"].append({"sku": sku, "qty": qty, "total": t})
        q["subtotal"] += t
    if q["subtotal"] >= THRESHOLDS["bulk"]:
        q["bulk"] = True
    if q["subtotal"] >= THRESHOLDS["free"]:
        q["free_shipping"] = True
    else:
        q["shipping"] = 500 if q["subtotal"] < 500 else 300
    q["boxes"] = (q["n"] + 1) // 2
    q["label"] = "Q-%04d-%s" % (q["subtotal"], "x" * q["boxes"])
    if q["subtotal"] % 2 == 0:
        q["even"] = True
    return q


small = quote([("c", 2)])                 #@row small: present={lines,subtotal,n,shipping,boxes,label,even} definite={lines,subtotal,n,shipping,boxes,label,even} closed
# 2 x 250 + 1000 = 1500: bulk but not free; 1500 is even; (2 + 1) // 2 = 1 box.
mid = quote([("a", 2), ("b", 1)])         #@row mid: present={lines,subtotal,n,bulk,shipping,boxes,label,even} definite={lines,subtotal,n,bulk,shipping,boxes,label,even} closed
# 3 x 1000 = 3000: bulk and free.
big = quote([("b", 3)])                   #@row big: present={lines,subtotal,n,bulk,free_shipping,boxes,label,even} definite={lines,subtotal,n,bulk,free_shipping,boxes,label,even} closed
# 75 x 3 = 225 is odd.
odd = quote([("c", 3)])                   #@row odd: present={lines,subtotal,n,shipping,boxes,label} definite={lines,subtotal,n,shipping,boxes,label} closed
line1 = mid["lines"][1]                   #@row line1: present={sku,qty,total} definite={sku,qty,total} closed
small["bulk"]                             #@ missing bulk
mid["free_shipping"]                      #@ missing free_shipping
big["shipping"]                           #@ missing shipping
odd["even"]                               #@ missing even
