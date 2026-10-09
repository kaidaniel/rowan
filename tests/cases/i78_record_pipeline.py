"""A list of record dicts transformed with map / filter / sorted and key lambdas, grouped
with setdefault, and summarised; element rows are observed through indexing and loops.

Expectations were written from the program text before running the engine."""

ORDERS = [
    {"id": 1, "cust": "ann", "total": 30, "rush": True},
    {"id": 2, "cust": "bob", "total": 5},
    {"id": 3, "cust": "ann", "total": 12},
]


def with_tax(o):
    return {**o, "tax": o["total"] * 0.2}


def big(o):
    return o["total"] > 10


def by_customer(orders):
    groups = {}
    for o in orders:
        groups.setdefault(o["cust"], []).append(o)
    return groups


taxed = list(map(with_tax, ORDERS))
first = taxed[0]                          #@row first: present={id,cust,total,rush,tax} definite={id,cust,total,rush,tax} closed
second = taxed[1]                         #@row second: present={id,cust,total,tax} definite={id,cust,total,tax} closed
large = list(filter(big, taxed))
# Which orders pass the filter is open: the element is the join of all three.
any_large = large[0]                      #@row any_large: present={id,cust,total,rush,tax} definite={id,cust,total,tax} closed
# The sort keys are known numbers (30, 5, 12), so the order is known: the top order is the
# first one, which has the rush flag.
ranked = sorted(taxed, key=lambda o: o["total"], reverse=True)
top = ranked[0]                           #@row top: present={id,cust,total,rush,tax} definite={id,cust,total,rush,tax} closed
groups = by_customer(ORDERS)              #@row groups: present={ann,bob} definite={ann,bob} closed
ann = groups["ann"][0]                    #@row ann: present={id,cust,total,rush} definite={id,cust,total,rush} closed
totals = {o["cust"]: o["total"] for o in ORDERS}  #@row totals: present={ann,bob} definite={ann,bob} closed
ids = [o["id"] for o in ORDERS if o.get("rush")]
summary = {"n": len(ORDERS), "customers": sorted(groups), "rush_ids": ids}  #@row summary: present={n,customers,rush_ids} definite={n,customers,rush_ids} closed
top["rush"]                               #@ ok rush
second["rush"]                            #@ missing rush
