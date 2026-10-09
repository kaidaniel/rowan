# isinstance / type dispatch normalising several input shapes into one record


def as_record(x):
    if isinstance(x, dict):
        rec = dict(x)
    elif isinstance(x, (list, tuple)):
        rec = {"id": x[0], "name": x[1]}
    elif isinstance(x, str):
        rec = {"name": x}
    else:
        rec = {"id": x}
    rec["kind"] = type(x).__name__
    _ = 0                                        #@row rec: present={id,name,kind,extra} definite={kind} closed
    k = rec["kind"]                              #@value k: exactly={dict,list,str,int}
    return rec


def describe(v):
    if type(v) is bool:
        return "flag"
    if isinstance(v, int):
        return "number"
    return "other"


if __name__ == "__main__":
    as_record({"id": 1, "extra": 2})
    as_record([2, "b"])
    r = as_record("c")                           #@row r: present={name,kind} definite={name,kind} closed
    as_record(7)
    try:
        r["id"]                                  #@ missing id
    except KeyError:
        pass
    t = type(r) is dict                          #@value t: exactly={True}
    d1 = describe(True)                          #@value d1: exactly={flag}
    d2 = describe(3)                             #@value d2: exactly={number}
    d3 = describe("3")                           #@value d3: exactly={other}
    sub = isinstance(True, int)                  #@value sub: exactly={True}
