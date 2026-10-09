registry = {"default": None}
for name in plugin_names:
    registry[name] = load(name)
registry["default"]                             #@ ok default
registry["other"]                               #@ none other

table = {}
for i in range(3):
    table[f"col_{i}"] = i
# Corrected on re-reading (r159 added a `range` model): range(3) is 0, 1, 2, so "col_1" is
# definitely stored; "none" recorded that range was not modelled.
table["col_1"]                                  #@ ok col_1

by_id = {item["id"]: item for item in items}
by_id[7]                                        #@ none 7

names = {"a": "alpha", "b": "beta"}
rev = {v: k for k, v in names.items()}
rev["alpha"]                                    #@ ok alpha
rev["a"]                                        #@ missing a
