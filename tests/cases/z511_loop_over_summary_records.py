# A loop over records made one per input item (by a comprehension or a loop of appends: one
# summary object stands for them all) takes one record per iteration: a store into it and a
# key a test proves hold for that record in the body; when nothing but the list holds the
# records and the loop went through all of it, every record has what the body stored.
import json
import os

raw = json.loads(os.environ.get("ROWAN_Z511", '[{"id": 1, "tag": "a"}, {"id": 2}]'))

recs = []
for r in raw:
    recs.append({"id": r["id"]})
for rec in recs:
    rec["seen"] = True
    #@row rec: present={id,seen} definite={id,seen} closed
for rec in recs:
    print(rec["seen"])  #@ ok seen

made = [{"id": r["id"], "tag": r.get("tag")} for r in raw]
for m in made:
    if "tag" in m:
        print(m["tag"])  #@ ok tag
    m["n"] = 0
for m in made:
    print(m["n"])  #@ ok n

# one record taken out of the list before the loop: it never went through the body
rest = [{"id": r["id"]} for r in raw]
extra = rest.pop()
for x in rest:
    x["done"] = True
print(extra["done"])  #@ possibly done
# (missing on every run: rowan does not tell `extra` apart from the records the loop marked;
# `possibly` is the sound answer it can give)
