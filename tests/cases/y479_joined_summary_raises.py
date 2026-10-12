"""A joined summary (every call past its site's budget: `tab=0`) raises what its body may raise,
reads that fail included: the caller's handler runs from the state where it raised."""
#@limits tab=0


def lookup(key):
    table = {"a": 1}
    return table[key]


def mark(d):
    d["state"] = "started"
    v = lookup(d["key"])
    d["state"] = "done"
    return v


try:
    lookup("z")
    after = "ran"
except KeyError:
    after = "caught"
x = after                                        #@value x: includes={caught}
job = {"key": "z"}
try:
    mark(job)
except KeyError:
    pass
st = job["state"]                                #@value st: includes={started}
