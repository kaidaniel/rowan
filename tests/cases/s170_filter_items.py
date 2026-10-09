"""Hypothesis: `filter(None, xs)` and `filter(pred, xs)` over known items keep exactly the
items that pass.

Expectations were written from the program text before running the engine."""

vals = ["a", "", "b", None]
kept = {v: 1 for v in filter(None, vals)}    #@row kept: present={a,b} definite={a,b} closed
rows = [{"ok": True, "id": "x"}, {"ok": False, "id": "y"}]
good = {r["id"]: r for r in filter(lambda r: r["ok"], rows)}  #@row good: present={x} definite={x} closed
good["y"]                                    #@ missing y
