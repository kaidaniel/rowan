# dict constructors: dict(zip(...)), dict(pairs), dict.fromkeys, dict(**kw), dict(mapping)

FIELDS = ("id", "name", "qty")


def from_row(values):
    rec = dict(zip(FIELDS, values))
    _ = 0                                        #@row rec: present={id,name,qty} definite={id,name} closed
    rec["id"]                                    #@ ok id
    return rec


def from_pairs(pairs):
    d = dict(pairs)
    return d


def blanks():
    d = dict.fromkeys(["a", "b"], 0)             #@row d: present={a,b} definite={a,b} closed
    v = d["b"]                                   #@value v: exactly={0}
    kw = dict(x=1, y=2)                          #@row kw: present={x,y} definite={x,y} closed
    both = dict(kw, z=3)                         #@row both: present={x,y,z} definite={x,y,z} closed
    lst = dict([("p", 1), ("q", 2)])             #@row lst: present={p,q} definite={p,q} closed
    lst["r"]                                     #@ missing r


if __name__ == "__main__":
    from_row((1, "a", 3))
    from_row((2, "b"))
    p = from_pairs([("k", 1), ("m", 2)])         #@row p: present={k,m} definite={k,m} closed
    q = from_pairs([])                           #@row q: present={} closed
    try:
        blanks()
    except KeyError:
        pass
