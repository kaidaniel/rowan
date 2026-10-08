"""**kwargs through layers, dict(**a, **b), update chains, defaults merging."""


def layer3(**kw):
    return {"l3": True, **kw}


def layer2(x, **kw):
    return layer3(x=x, **kw)


def layer1(*a, **kw):
    kw.setdefault("l1", 1)
    return layer2(*a, **kw)


r = layer1(1, y=2)
r["l1"]                                  #@ ok l1
r["l3"]                                  #@ ok l3
r["x"]                                   #@ ok x
r["y"]                                   #@ ok y
r["z"]                                   #@ missing z

DEFAULTS = {"host": "h", "port": 1}


def connect(**overrides):
    opts = dict(DEFAULTS, **overrides)
    return opts


c1 = connect(port=2)
c1["host"]                               #@ ok host
c1["port"]                               #@ ok port
c1["tls"]                                #@ missing tls
c2 = connect(tls=True)
c2["tls"]                                #@ ok tls

merged = {**DEFAULTS, **{"tls": False}, "extra": 1}
merged["tls"]                            #@ ok tls
merged["extra"]                          #@ ok extra
merged["nope"]                           #@ missing nope

upd = {"a": 1}
upd.update({"b": 2})
upd.update(c=3)
upd.update([("d", 4)])
upd["b"]                                 #@ ok b
upd["c"]                                 #@ ok c
upd["d"]                                 #@ ok d
upd["e"]                                 #@ missing e


def spread(d):
    return dict(d)


spread({"s": 1})["s"]                    #@ ok s
spread({"s": 1})["t"]                    #@ missing t
both = dict({"a": 1}, **{"b": 2})
both["a"]                                #@ ok a
both["b"]                                #@ ok b


def pass_through(**kw):
    return kw


pt = pass_through(**{"dyn": 1}, lit=2)
pt["dyn"]                                #@ ok dyn
pt["lit"]                                #@ ok lit
pt["x"]                                  #@ missing x
unknown_kw = pass_through(**external)
unknown_kw["anything"]                   #@ input anything
