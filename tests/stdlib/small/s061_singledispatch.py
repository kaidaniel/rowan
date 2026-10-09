# functools.singledispatch: the implementation registered for the argument's type runs (annotation or explicit type)

from functools import singledispatch


@singledispatch
def normalise(value):
    return {"kind": "other", "value": value}


@normalise.register
def _(value: dict):
    out = {"kind": "dict", "keys": sorted(value)}
    if "id" in value:
        out["id"] = value["id"]                    #@ ok id
    return out                                     #@row out: present={kind,keys,id} definite={kind,keys} closed


@normalise.register(list)
def _list(value):
    return {"kind": "list", "n": len(value)}


@normalise.register
def _(value: str):
    return {"kind": "str", "text": value.strip()}


def first_kind(items):
    res = normalise(items[0])
    #@row res: present={kind,keys,id,n} definite={kind} closed
    return res["kind"]


a = normalise({"id": 7, "x": 1})               #@row a: present={kind,keys,id} definite={kind,keys,id} closed
b = normalise([1, 2, 3])                        #@row b: present={kind,n} definite={kind,n} closed
c = normalise("  hi ")                          #@row c: present={kind,text} definite={kind,text} closed
d = normalise(4.5)                              #@row d: present={kind,value} definite={kind,value} closed
ka = a["kind"]                                  #@value ka: exactly={"dict"}
n = b["n"]                                      #@value n: exactly={3}
t = c["text"]                                   #@value t: exactly={"hi"}
e = normalise({"y": 2})                         #@row e: present={kind,keys} definite={kind,keys} closed
try:
    e["id"]                                     #@ missing id
except KeyError:
    pass

if __name__ == "__main__":
    k1 = first_kind([{"id": 1}])                #@value k1: exactly={"dict"}
    k2 = first_kind([[1], "x"])                 #@value k2: exactly={"list"}
