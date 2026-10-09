# map / filter / reversed with lambdas and named callbacks over records from several callers


def tag(r):
    return {**r, "tagged": True}


def pipeline(recs):
    tagged = list(map(tag, recs))
    kept = list(filter(lambda r: r["id"] > 0, tagged))
    back = list(reversed(kept))
    if back:
        first = back[0]
        _ = 0                                    #@row first: present={id,tagged,name} definite={id,tagged} closed
        first["tagged"]                          #@ ok tagged
    return back
#@schema param pipeline.recs['*']: always={id} only={id}


def names(recs):
    return list(map(lambda r: r["name"], recs))  #@ ok name ; missing name


def literals():
    sq = tuple(map(lambda x: x * x, [1, 2, 3]))   #@value sq: exactly={(1,4,9)}
    ev = tuple(filter(None, [0, 1, "", "a"]))     #@value ev: exactly={(1,"a")}
    rv = "".join(reversed("abc"))                #@value rv: exactly={"cba"}
    pairs = tuple(map(lambda a, b: a + b, "ab", "xy"))  #@value pairs: exactly={("ax","by")}
    return sq


if __name__ == "__main__":
    out = pipeline([{"id": 1, "name": "a"}, {"id": -1}])
    pipeline([{"id": 2}])
    pipeline([])
    names([{"name": "a"}])
    try:
        names([{"id": 1}])
    except KeyError:
        pass
    literals()
