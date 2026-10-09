# frozensets as dict keys and key-view set algebra (`keys() & {...}`, `<=`) on records

PERMS = {frozenset({"read"}): "viewer", frozenset({"read", "write"}): "editor"}


def role_of(perms):
    return PERMS.get(frozenset(perms), "custom")


def known_part(rec):
    keep = rec.keys() & {"id", "name"}
    out = {k: rec[k] for k in keep}
    _ = 0                                        #@row out: present={id,name} closed
    lacking = tuple(sorted({"id", "name"} - rec.keys()))
    if {"id", "name"} <= rec.keys():
        rec["name"]                              #@ ok name
    return out, lacking


if __name__ == "__main__":
    r1 = role_of(["write", "read"])              #@value r1: exactly={editor}
    r2 = role_of({"read"})                       #@value r2: exactly={viewer}
    r3 = role_of([])                             #@value r3: exactly={custom}
    o1, l1 = known_part({"id": 1, "name": "a", "x": 2})
    #@row o1: present={id,name} definite={id,name} closed
    o2, l2 = known_part({"id": 2})
    #@value l2: exactly={("name",)}
    o3, l3 = known_part({})
    #@row o3: present={} closed
    d = {frozenset("ab"): 1}
    v = d[frozenset("ba")]                       #@value v: exactly={1}
