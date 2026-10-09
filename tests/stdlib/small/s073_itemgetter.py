# operator.itemgetter with one key / several keys: as a sort key and applied directly to records

from operator import itemgetter


def names_by_age(people):
    ordered = sorted(people, key=itemgetter("age"))
    get_name = itemgetter("name")
    return [get_name(p) for p in ordered]
#@schema param names_by_age.people['*']: always={age,name} only={age,name}


def coords(points):
    xy = itemgetter("x", "y")
    return [xy(p) for p in points]


ns = names_by_age([{"name": "b", "age": 40}, {"name": "a", "age": 30, "city": "x"}])
n0 = ns[0]                                       #@value n0: exactly={"a"}
cs = coords([{"x": 1, "y": 2, "z": 3}])
c0 = cs[0]                                       #@value c0: exactly={(1, 2)}
g = itemgetter("k")
v = g({"k": "val"})                              #@value v: exactly={"val"}
try:
    g({"j": 1})                                  #@ missing k
except KeyError:
    pass
try:
    coords([{"x": 1}])                           #@ missing y
except KeyError:
    pass
t = itemgetter(1, 0)(["a", "b"])                 #@value t: exactly={("b", "a")}
top = sorted([("b", 2), ("a", 1)], key=itemgetter(1))[0]   #@value top: exactly={("a", 1)}
