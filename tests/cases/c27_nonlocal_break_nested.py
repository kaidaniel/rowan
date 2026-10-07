def build():
    d = {"a": 1}

    def extend():
        nonlocal d
        d = {**d, "b": 2}

    extend()
    return d


build()["b"]                                        #@ ok b
build()["c"]                                        #@ missing c

items = [{"id": 1, "tag": "x"}, {"id": 2}]
found = None
for it in items:
    if it["id"] == target:
        found = it
        break
found["id"]                                         #@ ok id
found["tag"]                                        #@ possibly tag

groups = {}
for rec in records:
    g = groups.setdefault(rec["kind"], {"count": 0, "items": []})
    g["count"] += 1                                 #@ ok count
    g["items"].append(rec)
    g["total"]                                      #@ missing total
for g in groups.values():
    g["count"]                                      #@ ok count
