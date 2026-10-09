# dict methods in helpers called with different key sets: copy / setdefault / pop / get / update


def normalize(rec):
    out = rec.copy()
    out.setdefault("status", "new")
    name = out.pop("nm", None)                   #@ none nm
    if name is not None:
        out["name"] = name
    _ = 0                                        #@row out: present={id,name,status,tags} definite={id,status} absent={nm} closed
    out["id"]                                    #@ ok id
    out["status"]                                #@ ok status
    label = out.get("name", "?")                 #@ none name
    return out


def with_defaults(rec, extra):
    d = {"active": True, "status": "draft"}
    d.update(rec)
    d.update(extra)
    _ = 0                                        #@row d: present={active,status,id,name,tags,owner} definite={active,status,id} closed
    d["active"]                                  #@ ok active
    n = len(d)                                   #@value n: exactly={4,5}
    return d


def show(r):
    return r["name"]                             #@ ok name ; missing name
#@schema param show.r: always={name} only={name}


if __name__ == "__main__":
    a = normalize({"id": 1, "nm": "ann"})
    b = normalize({"id": 2, "name": "bob", "status": "old"})
    c = normalize({"id": 3, "tags": []})
    with_defaults(a, {})
    with_defaults(c, {"owner": "kai"})
    with_defaults({"id": 9}, {"name": "x", "tags": [1]})
    for r in (a, b, c):
        try:
            show(r)
        except KeyError:
            pass
