"""Keys built from strings: case changes, affixes, formatting, replacements, and how they
flow through renames."""
raw = {"Name": 1, "AGE": 2, "e-mail": 3}
low = {k.lower(): v for k, v in raw.items()}
low["name"]                              #@ ok name
low["age"]                               #@ ok age
low["Name"]                              #@ missing Name
up = {k.upper(): v for k, v in raw.items()}
up["NAME"]                               #@ ok NAME
up["E-MAIL"]                             #@ ok E-MAIL
snake = {k.replace("-", "_"): v for k, v in raw.items()}
snake["e_mail"]                          #@ ok e_mail
snake["e-mail"]                          #@ missing e-mail
pref = {"col_" + k: v for k, v in raw.items()}
pref["col_Name"]                         #@ ok col_Name
pref["Name"]                             #@ missing Name
suf = {f"{k}_x": v for k, v in raw.items()}
suf["AGE_x"]                             #@ ok AGE_x
strip = {k.strip(): 1 for k in [" a ", "b"]}
strip["a"]                               #@ ok a
strip[" a "]                             #@ missing " a "
title = {k.title(): 1 for k in ["hello world"]}
title["Hello World"]                     #@ ok "Hello World"

col = "Name"
raw[col]                                 #@ ok Name
raw[col.lower()]                         #@ missing name
low[col.lower()]                         #@ ok name
for c in ("Name", "AGE"):
    raw[c]                               #@ ok Name ; ok AGE
    low[c]                               #@ missing Name ; missing AGE
    low[c.lower()]                       #@ ok name ; ok age

nums = {1: "a", "1": "b"}
nums[1]                                  #@ ok 1
nums["1"]                                #@ ok "1"
nums[str(1)]                             #@ ok "1"
nums[2]                                  #@ missing 2
i = 3
sq = {"k3": 1}
sq["k%d" % i]                            #@ any k3
sq["k" + str(i)]                         #@ ok k3
sq[f"k{i}"]                              #@ ok k3
sq[f"k{i + 1}"]                          #@ missing k4

template = "{}_id"
ids = {"user_id": 1}
ids[template.format("user")]             #@ ok user_id
ids[template.format("org")]              #@ missing org_id

parts = "a.b.c".split(".")
tree = {"a": {"b": {"c": 1}}}
tree[parts[0]]                           #@ ok a
