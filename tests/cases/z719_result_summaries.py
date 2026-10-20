# A call past its budget gives back what its function returns for any call (a result
# summary, from a run of the function with open parameters after any of the program's code
# ran): a dict display's keys, nested records and lists, an instance's attributes read, the
# literals among the values. What may be an object the function was given, one it reads from
# elsewhere (`self.cache`), or one it also puts where the program reads it back (a global
# list) stays unknown: a fresh record would not be the object the program has. (With
# `budget=0` a function's first call runs, the later ones are past its budget.)
#@limits budget=0


def make(n):
    return {"id": n, "name": "x", "tags": []}


def pick(row, extra):
    out = {"id": row["id"]}
    if extra:
        out["extra"] = extra
    return out


def nested():
    return {"a": {"b": 1}, "xs": [1, 2]}


REG = []


def kept():
    d = {"k": 1}
    REG.append(d)
    return d


def copy_of(row):
    return dict(row)


CONF = {"a": 1}


def conf_copy():
    return {**CONF}


class Box:
    def __init__(self, v):
        self.v = v
        self.cache = {"hit": 0}

    def info(self):
        return {"v": self.v, "n": 3}

    def the_cache(self):
        return self.cache


first = make(1)
second = make(2)
#@row second: present={id,name,tags} definite={id,name,tags} closed
nm = second["name"]                         #@ ok name
#@value nm: exactly={"x"}
sid = second["id"]                          #@ ok id
#@value sid: unknown
try:
    second["nope"]                          #@ missing nope
except KeyError:
    pass

p1 = pick({"id": 1}, None)
p2 = pick({"id": 2}, "e")
#@row p2: present={id,extra} definite={id} closed
p2["id"]                                    #@ ok id
p2["extra"]                                 #@ sound extra

n1 = nested()
n2 = nested()
inner = n2["a"]                             #@ ok a
#@row inner: present={b} definite={b} closed
bb = inner["b"]                             #@ ok b
#@value bb: exactly={1}
xs = n2["xs"]                               #@ ok xs
x0 = xs[0]
#@value x0: exactly={1}

k1 = kept()
k2 = kept()
#@row k2: present={} definite={} open
k2["k"]                                     #@ sound k

c1 = copy_of({"a": 1})
c2 = copy_of({"b": 2})
#@row c2: present={} definite={} open
c2["b"]                                     #@ sound b

cc1 = conf_copy()
cc2 = conf_copy()
#@row cc2: present={a,b} definite={a} closed
# (`b` is possible: the summary holds for any call, also those after `CONF["b"] = 2`)
CONF["b"] = 2
cc3 = conf_copy()
#@row cc3: present={a,b} definite={a} closed
cc3["b"]                                    #@ sound b

b1 = Box(1)
b2 = Box(2)
i1 = b1.info()
i2 = b2.info()
#@row i2: present={v,n} definite={v,n} closed
nn = i2["n"]                                #@ ok n
#@value nn: exactly={3}
t1 = b1.the_cache()
t2 = b2.the_cache()
#@row t2: present={} definite={} open
t2["hit"]                                   #@ sound hit
