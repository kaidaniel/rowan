# `next` of a generator raises where it asks for the item the generator fails to give: the
# handler sees what ran between the items; asked again in a later try, that try's handler
# catches it (the earlier one, which asked only for the first item, does not).
def failing(d):
    yield d["a"]
    yield d["b"]


def two(d):
    g = failing(d)
    seen = []
    try:
        a = next(g)
        seen.append(a)
        b = next(g)
        seen.append(b)
    except KeyError:
        return len(seen)
    return -1


n = two({"a": 1})
#@value n: includes={1}
#@value n: exactly={1}


def later(d):
    g = failing(d)
    try:
        a = next(g)
    except KeyError:
        return "first"
    seen = [a]
    try:
        next(g)
    except KeyError:
        return len(seen)
    return -1


m = later({"a": 1})
#@value m: includes={1}
#@todo value m: exactly={1}  (the first try's end takes the raise not asked for there as if it were)


def fine(d):
    g = failing(d)
    out = []
    try:
        out.append(next(g))
        out.append(next(g))
    except KeyError:
        return -1
    return sum(out)


s = fine({"a": 1, "b": 2})
#@value s: includes={3}
#@value s: exactly={3}
