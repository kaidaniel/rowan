# A program's own function named like a builtin is not the builtin: a test calling the
# program's `isinstance` or `hasattr` narrows nothing by what the builtin would say, and the
# program's `len` called by a function that does not run (past its budget) may change what
# it is given, as any of the program's functions may.
#@limits budget=6


def isinstance(x, t):
    return True


def hasattr(o, n):
    return True


x = 1
r = "none"
if isinstance(x, str):
    r = "str"
#@value r: exactly={"str"}

d = {}
q = "none"
if hasattr(d, "zzz"):
    q = "has"
#@value q: exactly={"has"}


def len(d):
    d["x"] = 1
    return 0


def fill(d):
    a = 1
    a = 2
    a = 3
    len(d)


fill({})
fill({})
fill({})
e = {}
fill(e)
n = 0
if "x" in e:
    n = 1
#@value n: includes={1}
#@todo value n: exactly={1}  (the last `fill` does not run: past its budget, what its `len` may do to e is known only as a change of any key)
