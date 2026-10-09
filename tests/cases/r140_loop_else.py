"""Hypothesis: the `else` of a `for` runs only when no `break` happened, decided per call for
known items; the same for a `while` whose test is decided on every iteration.

Expectations were written from the program text before running the engine."""


def find(items, want):
    res = {"want": want}
    for it in items:
        if it["id"] == want:
            res["found"] = it
            break
    else:
        res["missing"] = True
    return res


ITEMS = [{"id": 1, "v": "a"}, {"id": 2, "v": "b", "extra": 1}]
hit = find(ITEMS, 2)                         #@row hit: present={want,found} definite={want,found} closed
miss = find(ITEMS, 9)                        #@row miss: present={want,missing} definite={want,missing} closed
f = hit["found"]                             #@row f: present={id,v,extra} definite={id,v,extra} closed
hit["missing"]                               #@ missing missing
miss["found"]                                #@ missing found

state = {"i": 0}
while state["i"] < 5:
    state["i"] += 1
    if state["i"] == 3:
        state["hit"] = True
        break
else:
    state["exhausted"] = True
_ = 0                                        #@row state: present={i,hit} definite={i,hit} closed
state["exhausted"]                           #@ missing exhausted
