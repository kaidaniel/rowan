"""Hypothesis: in a loop over known items, a `continue` taken for some items (decided per
item) leaves the keys set after it present only for the others.

Expectations were written from the program text before running the engine."""

ITEMS = [{"id": 1, "skip": True}, {"id": 2}, {"id": 3}]
out = {}
for it in ITEMS:
    if it.get("skip"):
        continue
    out[it["id"]] = {"id": it["id"], "kept": True}
_ = 0                                     #@row out: present={2,3} definite={2,3} closed
out[1]                                    #@ missing 1
kept = out[2]                             #@row kept: present={id,kept} definite={id,kept} closed
