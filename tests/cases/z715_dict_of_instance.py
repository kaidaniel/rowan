"""`dict(x)` of an instance of the program's class that is no dict takes what CPython takes:
its pairs, by its `__iter__` (networkx's `DegreeView`); one with a `keys` method by `keys()`
and `x[k]` (CPython's dunders run, `builtin_dunder`). Before, it copied the instance's attributes as keys:
`d["c"]` was reported never set, `d` had a key "_data".

Expectations were written from the program text before running the engine."""


class Pairs:
    def __init__(self, data):
        self._data = data

    def __iter__(self):
        return iter(list(self._data.items()))


class Mapping:
    def __init__(self):
        self._data = {"x": 1}

    def keys(self):
        return ["x"]

    def __getitem__(self, k):
        return self._data[k]


d = dict(Pairs({"c": 3, "a": 1}))
c = d["c"]                                   #@ ok c
# (first written `sound` and open: `__iter__` runs as code, so its pairs are known exactly)
#@row d: present={a,c} definite={a,c} absent={_data} closed
m = dict(Mapping())
x = m["x"]                                   #@ sound x
#@todo ok x   (the keys `keys()` gives are taken, the dict is left open)
#@row m: present={x} definite={x} open
#@todo row m: present={x} definite={x} absent={_data} closed
