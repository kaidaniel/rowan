"""Parameter kinds: positional-only, keyword-only, defaults, *args, **kwargs, forwarding."""


def build(a, b=2, /, c=3, *rest, d, e=5, **extra):
    out = {"a": a, "b": b, "c": c, "d": d, "e": e}
    out["nrest"] = len(rest)
    for key, value in extra.items():
        out[key] = value
    return out


r1 = build(1, d=4)
r1["a"]                                  #@ ok a
r1["b"]                                  #@ ok b
r1["d"]                                  #@ ok d
r1["nrest"]                              #@ ok nrest
r1["zz"]                                 #@ missing zz
r2 = build(1, 2, 3, 9, 9, d=4, extra_key=1)
r2["extra_key"]                          #@ ok extra_key
r2["other"]                              #@ missing other
r3 = build(1, 2, c=7, d=4, **{"p": 1, "q": 2})
r3["p"]                                  #@ ok p
r3["q"]                                  #@ ok q
r3["r"]                                  #@ missing r


def forward(*args, **kwargs):
    return build(*args, **kwargs)


r4 = forward(1, d=1, k9=1)
r4["k9"]                                 #@ ok k9
r4["a"]                                  #@ ok a
r4["missing"]                            #@ missing missing


def defaults_are_shared(x, cache={}):
    cache[x] = True
    return cache


defaults_are_shared("m")
dd = defaults_are_shared("n")
dd["n"]                                  #@ ok n
dd["m"]                                  #@ any m


def kw_only(*, name, **rest):
    return {"name": name, **rest}


kw_only(name="x", tag=1)["tag"]          #@ ok tag
kw_only(name="x")["tag"]                 #@ missing tag


def takes_dict(d: dict, key: str = "a"):
    return d[key]


takes_dict({"a": 1})                     #@ ok a
takes_dict({"b": 1}, "b")                #@ ok b
takes_dict({"b": 1})                     #@ missing a
takes_dict({"a": 1}, key="c")            #@ missing c
