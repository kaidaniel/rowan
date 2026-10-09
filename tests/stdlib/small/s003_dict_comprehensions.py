# comprehension forms over records: key filters, renames, literal key lists, nested and set/list forms

RENAMES = {"nm": "name", "e": "email"}


def rename(rec):
    out = {RENAMES.get(k, k): v for k, v in rec.items()}
    _ = 0                                        #@row out: present={id,name,email,age} definite={id} absent={nm,e} closed
    return out


def pick(rec, wanted):
    return {k: rec[k] for k in wanted if k in rec}


def project(rec):
    p = pick(rec, ["id", "name"])
    _ = 0                                        #@row p: present={id,name} definite={id} closed
    p["id"]                                      #@ ok id
    blank = {k: None for k in ("a", "b", "c")}   #@row blank: present={a,b,c} definite={a,b,c} closed
    sq = {n: n * n for n in range(3)}            #@row sq: present={0,1,2} definite={0,1,2} closed
    v = sq[2]                                    #@value v: exactly={4}
    keys = tuple(sorted({k.upper() for k in blank}))    #@value keys: exactly={("A","B","C")}
    nested = {k: {"v": rec[k]} for k in ("id",)}
    inner = nested["id"]                         #@row inner: present={v} definite={v} closed
    return p


def strip_private(rec):
    out = {k: v for k, v in rec.items() if not k.startswith("_")}
    _ = 0                                        #@row out: present={id,name,email,age} definite={id} absent={_rev,_tmp} closed
    return out


if __name__ == "__main__":
    rows = [{"id": 1, "nm": "a"}, {"id": 2, "e": "x@y", "age": 4}, {"id": 3, "name": "c"}]
    for r in rows:
        rr = rename(r)
        project(rr)
        strip_private({**rr, "_rev": 1})
    strip_private({"id": 5, "_tmp": 0})
