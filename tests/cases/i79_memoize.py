"""A memoising decorator with a closure-held cache keyed by the argument; the wrapped
functions return dicts; the cache's keys are the literal arguments used.

Expectations were written from the program text before running the engine."""
import functools


def memoize(fn):
    cache = {}

    @functools.wraps(fn)
    def wrapper(key):
        if key in cache:
            return cache[key]
        result = fn(key)
        cache[key] = result
        return result

    wrapper.cache = cache
    return wrapper


@memoize
def profile(uid):
    return {"uid": uid, "name": "user%d" % uid}


@memoize
def settings(uid):
    s = {"uid": uid, "theme": "dark"}
    if uid == 1:
        s["admin"] = True
    return s


p1 = profile(1)                           #@row p1: present={uid,name} definite={uid,name} closed
p2 = profile(2)
p1_again = profile(1)                     #@row p1_again: present={uid,name} definite={uid,name} closed
pc = profile.cache                        #@row pc: present={1,2} definite={1,2} closed
s1 = settings(1)                          #@row s1: present={uid,theme,admin} definite={uid,theme,admin} closed
s2 = settings(2)                          #@row s2: present={uid,theme} definite={uid,theme} closed
sc = settings.cache                       #@row sc: present={1,2} definite={1,2} closed
s2["admin"]                               #@ missing admin
pc[3]                                     #@ missing 3
