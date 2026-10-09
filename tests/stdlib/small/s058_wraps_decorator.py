# a functools.wraps decorator forwarding *args / **kwargs: the wrapped function sees the caller's dicts

import functools

CALLS = []


def logged(fn):
    @functools.wraps(fn)
    def wrapper(*args, **kwargs):
        CALLS.append(fn.__name__)
        result = fn(*args, **kwargs)
        return result
    return wrapper


@logged
def describe(user, *, verbose=False):
    out = {"name": user["name"]}                   #@ ok name
    if verbose:
        out["email"] = user["email"]               #@ ok email
    return out
#@schema param describe.user: always={name} sometimes={email} only={name}


@logged
def merge(base, **overrides):
    out = {**base, **overrides}
    return out


d1 = describe({"name": "ann", "email": "a@x"}, verbose=True)   #@row d1: present={name,email} definite={name,email} closed
d2 = describe({"name": "bob", "age": 4})                       #@row d2: present={name} definite={name} closed
m = merge({"a": 1}, b=2, c=3)                                   #@row m: present={a,b,c} definite={a,b,c} closed
nm = describe.__name__                                          #@value nm: exactly={"describe"}
k = len(CALLS)                                                  #@value k: exactly={3}
try:
    d2["email"]                                                 #@ missing email
except KeyError:
    pass
