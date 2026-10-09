"""Hypothesis: `any` / `all` over generators on known items are decided (including the
empty list: `all([])` is True, `any([])` is False).

Expectations were written from the program text before running the engine."""


def summarize(items):
    s = {"n": len(items)}
    if all(it["q"] > 0 for it in items):
        s["positive"] = True
    if any(it["q"] > 1 for it in items):
        s["big"] = True
    if any("x" in it for it in items):
        s["has_x"] = True
    return s


r = summarize([{"q": 1}, {"q": 2}])          #@row r: present={n,positive,big} definite={n,positive,big} closed
e = summarize([])                            #@row e: present={n,positive} definite={n,positive} closed
w = summarize([{"q": 0, "x": 1}])            #@row w: present={n,has_x} definite={n,has_x} closed
r["has_x"]                                   #@ missing has_x
