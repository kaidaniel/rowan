"""**kwargs forwarded through three layers with setdefault, pop, `dict(kw, extra=...)` and
`*args`; the final record is built with a `**kw` spread.

Expectations were written from the program text before running the engine."""


def base(**kw):
    return {"base": True, **kw}


def mid(x, **kw):
    kw.setdefault("mid", x)
    extra = kw.pop("drop", None)
    r = base(**kw)
    if extra is not None:
        r["dropped"] = extra
    return r


def top(*args, **kw):
    return mid(*args, **dict(kw, top=1))


r1 = top(1, a=2)                          #@row r1: present={base,a,mid,top} definite={base,a,mid,top} closed
r2 = top(2, drop="d")                     #@row r2: present={base,mid,top,dropped} definite={base,mid,top,dropped} closed
r3 = top(3, mid="m", b=1)                 #@row r3: present={base,mid,top,b} definite={base,mid,top,b} closed
r1["dropped"]                             #@ missing dropped
r2["drop"]                                #@ missing drop
r2["dropped"]                             #@ ok dropped
