"""A generator's caller sees the objects it changed as the join of their states before the
call, at each yield and at its end. Twelve records are toggled between yields (`busy` added,
then deleted); the outer `while True:` is unrolled, 16 rounds of 24 yields. Each yield state
differs from the previous one only at the records it toggled, and their rows take two
values: joining only what a structural diff against the previous yield names, and each
distinct row once, the suspension join adds almost nothing to the body's own joins (12 rows
× 384 yields × about 2 for the branches and the consumer's loop, with a margin). Joining
every touched row with every yield state doubled the row joins."""
#@joins <= 12000
import os


def toggle(a, b, c, d, e, f, g, h, i, j, k, l):
    n = 0
    while True:
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n
        a["busy"] = str(n); b["busy"] = str(n); c["busy"] = str(n); d["busy"] = str(n); e["busy"] = str(n); f["busy"] = str(n); g["busy"] = str(n); h["busy"] = str(n); i["busy"] = str(n); j["busy"] = str(n); k["busy"] = str(n); l["busy"] = str(n)
        n += 1
        yield n
        del a["busy"]; del b["busy"]; del c["busy"]; del d["busy"]; del e["busy"]; del f["busy"]; del g["busy"]; del h["busy"]; del i["busy"]; del j["busy"]; del k["busy"]; del l["busy"]
        n += 1
        yield n


a = {"id": "a"}
b = {"id": "b"}
c = {"id": "c"}
d = {"id": "d"}
e = {"id": "e"}
f = {"id": "f"}
g = {"id": "g"}
h = {"id": "h"}
i = {"id": "i"}
j = {"id": "j"}
k = {"id": "k"}
l = {"id": "l"}
for k in toggle(a, b, c, d, e, f, g, h, i, j, k, l):
    if k > int(os.environ.get("STOP", "3")):
        break
print(a, l)  #@row a: present={id,busy} definite={id} closed
