"""Closures taking closures, returning lambdas, decorators with arguments, registries of
callables, partial application."""
import functools


def compose(f, g):
    return lambda x: f(g(x))


def add_key(k):
    def adder(d):
        return {**d, k: True}
    return adder


both = compose(add_key("a"), add_key("b"))
r = both({"base": 1})                    #@row r: present={base,a,b} definite={base,a,b} closed
r["a"]                                   #@ ok a
r["b"]                                   #@ ok b
r["c"]                                   #@ missing c


def apply_all(fns, d):
    for fn in fns:
        d = fn(d)
    return d


r2 = apply_all([add_key("x"), add_key("y")], {})
r2["x"]                                  #@ ok x
r2["y"]                                  #@ ok y
r2["z"]                                  #@ missing z


def with_default(key, default):
    def deco(fn):
        @functools.wraps(fn)
        def wrapper(*args, **kwargs):
            out = fn(*args, **kwargs)
            out.setdefault(key, default)
            return out
        return wrapper
    return deco


@with_default("retries", 3)
@with_default("timeout", 10)
def make_cfg(host):
    return {"host": host}


cfg = make_cfg("h")
cfg["host"]                              #@ ok host
cfg["retries"]                           #@ ok retries
cfg["timeout"]                           #@ ok timeout
cfg["port"]                              #@ missing port

HANDLERS = {}


def register(name):
    def deco(fn):
        HANDLERS[name] = fn
        return fn
    return deco


@register("a")
def handle_a(p):
    return {"kind": "a", "a": p["a"]}    #@ input a


@register("b")
def handle_b(p):
    return {"kind": "b"}


res = HANDLERS["a"](payload)             #@ input a
res["kind"]                              #@ ok kind
res["a"]                                 #@ ok a
res["b"]                                 #@ missing b
res2 = HANDLERS[which](payload)          #@ none kind
res2["kind"]                             #@ ok kind
res2["a"]                                #@ possibly a


def make_counter():
    counts = {}

    def bump(k):
        counts[k] = counts.get(k, 0) + 1

    def get(k):
        return counts[k]

    return bump, get


bump, get = make_counter()
bump("hits")
get("hits")                              #@ ok hits
get("misses")                            #@ missing misses

add_z = functools.partial(lambda d, k: {**d, k: 1}, {"q": 0})
pz = add_z("z")
pz["z"]                                  #@ ok z
pz["q"]                                  #@ ok q
pz["w"]                                  #@ missing w


def pipeline(*steps):
    def run(d):
        for s in steps:
            d = s(d)
        return d
    return run


run = pipeline(lambda d: {**d, "s1": 1}, lambda d: {**d, "s2": 2})
out = run({"in": 1})
out["s1"]                                #@ ok s1
out["s2"]                                #@ ok s2
out["in"]                                #@ ok in
out["s3"]                                #@ missing s3


def returns_lambda_taking_lambda(d):
    return lambda f: f(d)


rl = returns_lambda_taking_lambda({"deep": 1})
rl(lambda d: d["deep"])                  #@ ok deep
rl(lambda d: d["shallow"])               #@ missing shallow
