"""An uncalled function reading nested paths of its parameter: a schema per path; its own
result is exact.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def summarise(order):
    total = 0
    for line in order["lines"]:
        total += line["qty"] * line["price"]
    cust = order["customer"]
    tier = cust.get("tier", "basic")
    out = {"total": total, "tier": tier}
    _ = 0                                    #@row out: present={total,tier} definite={total,tier} closed
    return out
#@schema param summarise.order: always={lines,customer}
#@schema param summarise.order['lines']['*']: always={qty,price}
