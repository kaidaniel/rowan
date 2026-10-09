"""Hypothesis: a mutable class attribute is one object shared by every instance (a store via
`self.shared` is seen through the class and the other instance), while a dict made in
`__init__` is per instance.

Expectations were written from the program text before running the engine."""


class Store:
    shared = {}

    def __init__(self):
        self.own = {}

    def put(self, k):
        self.shared[k] = True
        self.own[k] = True


a = Store()
b = Store()
a.put("x")
b.put("y")
sa = a.shared                                #@row sa: present={x,y} definite={x,y} closed
sc = Store.shared                            #@row sc: present={x,y} definite={x,y} closed
oa = a.own                                   #@row oa: present={x} definite={x} closed
ob = b.own                                   #@row ob: present={y} definite={y} closed
a.own["y"]                                   #@ missing y
Store.shared["z"]                            #@ missing z
