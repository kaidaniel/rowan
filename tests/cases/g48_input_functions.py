"""Function inputs used to index the result of a function that is itself an input."""


def lookup(get, key):
    return get()[key]


def by_name(fn, rec):
    return fn(rec)["name"]


def make_rec():
    return {"name": "x", "id": 1}


lookup(make_rec, "name")                 #@ ok name
lookup(make_rec, "zzz")                  #@ missing zzz
by_name(lambda r: r, {"name": "y"})      #@ ok name
by_name(lambda r: {"name": r["n"]}, {"n": 1})      #@ ok name ; ok n
by_name(lambda r: {"other": 1}, {})      #@ missing name


def apply_key(fn, d, k):
    return fn(d)[k]


apply_key(dict, {"a": 1}, "a")           #@ ok a
apply_key(dict, {"a": 1}, "b")           #@ missing b


def dispatch(table, kind, payload):
    return table[kind](payload)


T = {"inc": lambda p: {"v": p["v"] + 1}, "neg": lambda p: {"v": -p["v"], "neg": True}}
dispatch(T, "inc", {"v": 1})["v"]        #@ ok v
dispatch(T, "neg", {"v": 1})["neg"]      #@ ok neg
dispatch(T, "inc", {"v": 1})["neg"]      #@ missing neg
dispatch(T, "inc", {"w": 1})             #@ missing v


def pick(getter, source, field):
    rec = getter(source)
    return rec[field]


class Repo:
    def __init__(self, rows):
        self.rows = rows

    def first(self):
        return self.rows[0]


repo = Repo([{"id": 7, "name": "n"}])
pick(Repo.first, repo, "id")             #@ ok id
pick(Repo.first, repo, "nope")           #@ missing nope
pick(lambda r: r.first(), repo, "name")  #@ ok name
