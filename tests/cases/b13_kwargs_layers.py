def inner(a, b=1, **rest):
    return {"a": a, "b": b, **rest}


def api(**kw):
    kw.setdefault("tag", "x")
    return inner(**kw)


r = api(a=1, c=2)                    #@row r: present={a,b,c,tag} definite={a,b,c,tag} closed
r["c"]                               #@ ok c
r["tag"]                             #@ ok tag
r["d"]                               #@ missing d
api(a=1)["c"]                        #@ missing c
