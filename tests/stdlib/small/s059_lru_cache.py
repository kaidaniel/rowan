# lru_cache / cache: a cached call returns the same dict object for the same literal argument

from functools import cache, lru_cache


@lru_cache(maxsize=None)
def settings(env):
    base = {"env": env, "debug": False}
    if env == "dev":
        base["debug"] = True
        base["reload"] = True
    return base


@cache
def fib(n):
    return n if n < 2 else fib(n - 1) + fib(n - 2)


def describe(cfg):
    s = settings(cfg["env"])
    #@row s: present={env,debug,reload} definite={env,debug} closed
    return s["debug"]
#@schema param describe.cfg: always={env} only={env}


dev = settings("dev")                         #@row dev: present={env,debug,reload} definite={env,debug,reload} closed
prod = settings("prod")                       #@row prod: present={env,debug} definite={env,debug} closed
same = settings("dev") is dev                 #@value same: exactly={True}
dbg = prod["debug"]                           #@value dbg: exactly={False}
f = fib(10)                                   #@value f: exactly={55}
hits = settings.cache_info().hits             #@value hits: exactly={1}
try:
    prod["reload"]                            #@ missing reload
except KeyError:
    pass

if __name__ == "__main__":
    a = describe({"env": "dev"})              #@value a: exactly={True}
    b = describe({"env": "test", "x": 1})     #@value b: exactly={False}
