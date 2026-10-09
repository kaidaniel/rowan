"""Hypothesis: `x = d.get("a") and d["a"]["b"]` evaluates the right side only when the
left is truthy, so a nested access on a present key is fine and a missing-key path is not
taken when the key is absent.

Expectations were written from the program text before running the engine."""

d = {"a": {"b": {"deep": 1}}}
e = {}
x = d.get("a") and d["a"]["b"]            #@row x: present={deep} definite={deep} closed
# e.get("a") is None, so the right side never runs: no access to report.
y = e.get("a") and e["a"]["b"]            #@ none a
z = {"x": x, "y": y}                      #@row z: present={x,y} definite={x,y} closed
x["other"]                                #@ missing other
