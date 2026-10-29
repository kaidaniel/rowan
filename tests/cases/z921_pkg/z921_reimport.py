"""A module whose import raised is not in `sys.modules`: importing it again runs its code
again (and raises again), at module level and in a function called later; `from … import`
too.

Expectations were written from the program text before running the engine."""
try:
    import failing
    a = failing.Y
except ImportError:
    a = "failed"
va = a                                       #@value va: exactly={2, "failed"}

try:
    import failing
    b = failing.Y
except ImportError:
    b = "again"
vb = b                                       #@value vb: exactly={2, "again"}

try:
    from failing import X
    c = X
except ImportError:
    c = "from"
vc = c                                       #@value vc: exactly={1, "from"}


def later():
    try:
        import failing
        return failing.Y
    except ImportError:
        return "later"


vd = later()                                 #@value vd: exactly={2, "later"}


try:
    import latename
    e1 = "loaded"
except NameError:
    e1 = "NameError"


def again():
    # (a new module object: the names the first run bound are not there)
    try:
        import latename
        return "loaded"
    except NameError:
        return "NameError again"


ve = again()                                 #@value ve: includes={"NameError again"}
