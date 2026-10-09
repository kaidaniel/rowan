"""Hypothesis: subscription on an instance of a class with `__getitem__` / `__setitem__` /
`__contains__` runs those methods: a missing key is reported inside `__getitem__` for the
calling line, a store reaches the wrapped dict, and `in` uses `__contains__`.

Expectations were written from the program text before running the engine."""


class Wrapper:
    def __init__(self, data):
        self.data = data

    def __getitem__(self, key):
        return self.data[key]

    def __setitem__(self, key, value):
        self.data[key] = value

    def __contains__(self, key):
        return key in self.data


w = Wrapper({"a": {"inner": 1}, "b": 2})
x = w["a"]                                   #@row x: present={inner} definite={inner} closed
w["c"] = 3
d = w.data                                   #@row d: present={a,b,c} definite={a,b,c} closed
if "zz" in w:
    w["zz"]                                  #@ none zz
w["zz"]                                      #@ missing zz
