"""A name bound on some paths only may be unbound where it is read: NameError at module level,
UnboundLocalError in a function. The value is still what the binding
paths gave, and after the read the name is bound (the program went on)."""
import sys

if len(sys.argv) > 2:
    x = {"a": 1}
try:
    y = x["a"]
#@ possibly x
    y2 = x["a"]
#@ none x
except NameError:
    y = None
yv = y
#@value yv: exactly={1, None}


def pick(flag):
    if flag:
        z = "on"
    return z
#@ possibly z


def both(flag):
    if flag:
        w = 1
    else:
        w = 2
    return w
#@ none w


def never():
    if False:
        q = 1
    return q
#@ missing q


def loop_known():
    for it in [1, 2, 3]:
        last = it
    return last
#@ none last


try:
    r1 = pick(len(sys.argv) > 2)
except UnboundLocalError:
    r1 = None
r1v = r1
#@value r1v: exactly={"on", None}
r2 = both(len(sys.argv) > 2)
#@value r2: exactly={1, 2}
r3 = loop_known()
#@value r3: exactly={3}
try:
    r4 = never()
except UnboundLocalError:
    r4 = None
#@value r4: exactly={None}

mode = "fast" if len(sys.argv) > 3 else None
if mode == "fast":
#@ none mode
    speed = 2


def loop_last(items):
    for it in items:
        last = it
    return last
#@ possibly last


def parsed(text):
    try:
        cfg = int(text)
    except ValueError:
        pass
    return cfg
#@ possibly cfg


def countdown(n):
    while n > 0:
        r = n
        n -= 1
    return r
#@ possibly r


def deleted(flag):
    v = 1
    if flag:
        del v
    return v
#@ possibly v


def same_test(flag):
    if flag:
        u = 1
    if flag:
        return u
#@ none u
    return 0


for fn, arg in [(loop_last, sys.argv), (parsed, sys.argv[0]), (countdown, len(sys.argv)), (deleted, len(sys.argv) > 1), (same_test, len(sys.argv) > 1)]:
    try:
        fn(arg)
    except NameError:
        pass
