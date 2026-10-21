# A generator expression the next statement gives whole to `sum` / `sorted` / `list` runs where
# it is written (nothing runs between); in a try, a raise in its items still leaves the name
# bound, as in Python, where the items run at the next statement.
def total(xs):
    items = (x * 2 for x in xs)
    return sum(items)


t = total([1, 2])
#@value t: exactly={6}


def ordered(d):
    keys = (k.upper() for k in d)
    for k in sorted(keys):
        d[k] = 1
    return d


o = ordered({"b": 0, "a": 0})
#@row o: present={a,b,A,B} definite={a,b,A,B} closed


def guarded(d):
    try:
        vals = (d[k] for k in ["a", "b"])
        out = list(vals)
    except KeyError:
        out = vals is not None
    return out


g = guarded({"a": 1})
#@value g: exactly={True}
# (a generator whose body surely raises before its end: `list` of it raises there, nothing after it runs)
