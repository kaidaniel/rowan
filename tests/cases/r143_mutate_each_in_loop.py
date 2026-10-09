"""Hypothesis: a store on the loop variable over a known list of dicts reaches each element;
a store guarded by a per-element decided test reaches only the elements that pass.

Expectations were written from the program text before running the engine."""

rows = [{"id": 1}, {"id": 2, "tag": "t"}]
for r in rows:
    r["seen"] = True
first = rows[0]                              #@row first: present={id,seen} definite={id,seen} closed
second = rows[1]                             #@row second: present={id,tag,seen} definite={id,tag,seen} closed
for r in rows:
    if r.get("tag"):
        r["tagged"] = True
rows[1]["tagged"]                            #@ ok tagged
rows[0]["tagged"]                            #@ missing tagged
