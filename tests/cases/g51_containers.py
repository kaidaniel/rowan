"""Records inside containers: lists, tuples, sets, nested dicts, zip / enumerate / items,
sorted, append in loops, indexing with constants and variables."""
rows = [{"id": 1, "name": "a"}, {"id": 2}]
rows[0]["name"]                          #@ ok name
rows[1]["name"]                          #@ missing name
rows[-1]["id"]                           #@ ok id
rows[i]["id"]                            #@ ok id
rows[i]["name"]                          #@ possibly name

pair = ({"left": 1}, {"right": 2})
l, r = pair
l["left"]                                #@ ok left
r["left"]                                #@ missing left
pair[1]["right"]                         #@ ok right

nested = {"outer": {"inner": {"leaf": 1}}}
nested["outer"]["inner"]["leaf"]         #@ ok outer ; ok inner ; ok leaf
nested["outer"]["leaf"]                  #@ missing leaf
nested["outer"]["inner"]["x"] = 2
nested["outer"]["inner"]["x"]            #@ ok x

acc = []
for n in range(k):
    acc.append({"n": n, "sq": n * n})
for item in acc:
    item["sq"]                           #@ ok sq
    item["cube"]                         #@ missing cube

mixed = [{"a": 1}, {"b": 2}]
for m in mixed:
    m["a"]                               #@ possibly a
    m["c"]                               #@ missing c

keys = ["x", "y"]
vals = [{"v": 1}, {"v": 2}]
for key, val in zip(keys, vals):
    val["v"]                             #@ ok v
    val["w"]                             #@ missing w
for idx, val in enumerate(vals):
    val["v"]                             #@ ok v
table = dict(zip(keys, vals))
table["x"]["v"]                          #@ ok x ; ok v
table["z"]                               #@ missing z

d = {"p": {"q": 1}, "r": {"q": 2, "s": 3}}
for name, sub in d.items():
    sub["q"]                             #@ ok q
    sub["s"]                             #@ possibly s
for sub in d.values():
    sub["q"]                             #@ ok q
for name in d:
    d[name]["q"]                         #@ ok q
for name in sorted(d):
    d[name]["q"]                         #@ ok q
    d[name]["t"]                         #@ missing t

lol = [[{"deep": 1}], [{"deep": 2}]]
lol[0][0]["deep"]                        #@ ok deep
lol[1][0]["shallow"]                     #@ missing shallow
for inner in lol:
    for cell in inner:
        cell["deep"]                     #@ ok deep

sdicts = {frozenset({"a"}): {"fs": 1}}
first_row = rows[0]
copy_rows = list(rows)
copy_rows[0]["id"]                       #@ ok id
copy_rows[0]["zz"]                       #@ missing zz
s = sorted(rows, key=lambda rr: rr["id"])  #@ ok id
s[0]["id"]                               #@ ok id
s[0]["name"]                             #@ possibly name
