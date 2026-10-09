# types.SimpleNamespace built from records (attributes = keys) and a read-only MappingProxyType view
from types import MappingProxyType, SimpleNamespace


def to_ns(d):
    ns = SimpleNamespace(**d)
    ns.loaded = True
    _ = 0                                        #@row ns: present={host,port,name,loaded} definite={loaded} closed
    p = getattr(ns, "port", None)                #@ none port
    return ns


def frozen_view(d):
    view = MappingProxyType(d)
    ro = False
    try:
        view["x"] = 1
    except TypeError:
        ro = True
    d["late"] = 1
    _ = 0                                        #@row view: present={a,late} definite={late} closed
    return view, ro


if __name__ == "__main__":
    a = to_ns({"host": "h", "port": 1})          #@row a: present={host,port,loaded} definite={host,port,loaded} closed
    b = to_ns({"name": "n"})
    try:
        b.port                                   #@ missing port
    except AttributeError:
        pass
    va = vars(a)                                 #@row va: present={host,port,loaded} definite={host,port,loaded} closed
    v1, ro = frozen_view({"a": 1})
    #@value ro: exactly={True}
    v2, _ro = frozen_view({})
    n = len(v1)                                  #@value n: exactly={2}
    try:
        v2["a"]                                  #@ missing a
    except KeyError:
        pass
