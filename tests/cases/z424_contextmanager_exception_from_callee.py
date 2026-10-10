# an exception raised in a callee of a @contextmanager's with body reaches the generator's handler (its names)
from contextlib import contextmanager


@contextmanager
def tracked(registry, name):
    entry = {"name": name, "state": "running"}
    registry[name] = entry
    try:
        yield entry
        entry["state"] = "done"
    except KeyError as e:
        entry["state"] = "failed"
        entry["missing"] = e.args[0]
        raise


def lookup(key):
    table = {"a": 1}
    return table[key]


def run(registry, key):
    with tracked(registry, key):
        v = lookup(key)
    return v


reg = {}
try:
    run(reg, "z")
except KeyError:
    pass
e = reg["z"]
#@row e: present={name,state,missing} definite={name,state,missing} closed
st = e["state"]                                      #@value st: exactly={failed}
m = e["missing"]                                     #@value m: exactly={z}
r2 = {}
ok = run(r2, "a")                                    #@value ok: exactly={1}
e2 = r2["a"]
#@row e2: present={name,state} definite={name,state} closed
