# An unknown of a builtin type joined with None keeps its type beside None (`None | int(?)`, not
# `None | ?`), and a test that removes None leaves the typed unknown (`int(?)`). Joined with a
# literal of another type, or an unknown of another type, it has no type.
import os
import sys


level = None
if os.environ.get("NO_LEVEL") is None:
    level = int(os.environ.get("LEVEL", "5"))
given = level                                       #@value given: shows={None, 5} types={int}
inc = level + 1                                     #@value inc: shows={6} types={int}
if level is not None:
    lv = level                                      #@value lv: shows={5} types={int}
    double = level * 2                              #@value double: shows={10} types={int}
else:
    lv0 = level                                     #@value lv0: exactly={None}
if level:
    t = level                                       #@value t: shows={5} types={int}

# a literal of the type beside it stays, None goes
count = None
if os.environ.get("NO_COUNT") is None:
    count = 0
if os.environ.get("COUNT"):
    count = int(os.environ["COUNT"])
both = count                                        #@value both: shows={None, 0} types={int}
if count is not None:
    some = count                                    #@value some: shows={0} types={int}


def parse(s):
    if not s:
        return None
    return s.strip()


p = parse(os.environ.get("NAME", ""))               #@value p: shows={None} types={str}
if p is not None:
    q = p.upper()                                   #@value q: types={str}

# of another type: no type
w = None
if os.environ.get("W_INT"):
    w = int(os.environ["W_INT"])
elif os.environ.get("NO_W") is None:
    w = "x"
mixed = w                                           #@value mixed: untyped; mixed: shows={None, "x"}
if w is not None:
    w2 = w                                          #@value w2: untyped; w2: shows={"x"}


def longest(names):
    best = None
    for n in names:
        k = len(n)
        if best is None or k > best:
            best = k
    return best


top = longest(sys.argv)                             #@value top: shows={None} types={int}


def scaled(x: int, flag: bool):
    r = None
    if flag:
        r = x * 2
    out = r                                         #@general out: shows={None} types={int}
    if out is None:
        return 0
    return out                                      #@general out: shows={} types={int}
