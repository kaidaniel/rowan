"""Hypothesis: a closure that mutates a dict from its enclosing scope, and a counter via
`nonlocal`, are seen by the enclosing function after the closure ran (through a cell).

Expectations were written from the program text before running the engine."""


def make():
    seen = {}
    count = 0

    def visit(key):
        nonlocal count
        count += 1
        seen[key] = count
        return seen

    def report():
        return {"seen": dict(seen), "count": count}

    return visit, report


visit, report = make()
visit("a")
visit("b")
r = report()                              #@row r: present={seen,count} definite={seen,count} closed
seen = r["seen"]                          #@row seen: present={a,b} definite={a,b} closed
seen["c"]                                 #@ missing c
