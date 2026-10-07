import copy

a = {"x": 1}
b = a
b["y"] = 2
a["y"]                               #@ ok y
c = dict(a)
c["z"] = 3
a["z"]                               #@ missing z
c["z"]                               #@ ok z
n = {"inner": a}
copy.copy(n)["inner"]["w"] = 1
a["w"]                               #@ ok w
copy.deepcopy(n)["inner"]["q"] = 1
a["q"]                               #@ missing q
