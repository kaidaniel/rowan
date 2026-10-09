# collections.namedtuple: fields, defaults, _asdict / _replace / _fields / _field_defaults
from collections import namedtuple

Point = namedtuple("Point", "x y")
Row = namedtuple("Row", ["id", "name", "tags"], defaults=[None, ()])


def make(rec):
    r = Row(**rec)                               #@row r: present={id,name,tags} definite={id,name,tags} closed
    return r


def describe(r):
    d = r._asdict()                              #@row d: present={id,name,tags} definite={id,name,tags} closed
    r2 = r._replace(name="x")                    #@row r2: present={id,name,tags} definite={id,name,tags} closed
    n = r2.name                                  #@value n: exactly={x}
    r.tags                                       #@ ok tags
    return d


if __name__ == "__main__":
    a = make({"id": 1})
    b = make({"id": 2, "name": "b", "tags": ("t",)})
    describe(a)
    describe(b)
    t = a.tags                                   #@value t: exactly={()}
    try:
        a.email                                  #@ missing email
    except AttributeError:
        pass
    rejected = False
    try:
        a._replace(bogus=1)
    except (TypeError, ValueError):
        rejected = True
    #@value rejected: exactly={True}
    f = Row._fields                              #@value f: exactly={("id","name","tags")}
    dflt = Row._field_defaults                   #@row dflt: present={name,tags} definite={name,tags} closed
    P3 = namedtuple("P3", Point._fields + ("z",))
    p = P3(1, 2, 3)                              #@row p: present={x,y,z} definite={x,y,z} closed
    pd_ = Point(1, 2)._asdict()                  #@row pd_: present={x,y} definite={x,y} closed
