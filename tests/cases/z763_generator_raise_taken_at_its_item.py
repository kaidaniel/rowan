# A generator's raise reaches its loop where the loop asks for the item the generator fails to
# give: the handler sees what the loop's body did with the items before (and nothing after the
# loop runs on that path); a loop that breaks off before that item never sees the raise. So for
# a comprehension's items.
def failing(d):
    yield d["a"]
    yield d["b"]


def first_fails(d):
    yield d["x"]


def loop_first(d):
    seen = []
    try:
        for v in first_fails(d):
            seen.append(v)
    except KeyError:
        return len(seen)
    return -1


n1 = loop_first({})
#@value n1: exactly={0}


def rows(d, n):
    for i in range(n):
        yield i
    yield d["end"]


def counted(d, n):
    count = 0
    try:
        for v in rows(d, n):
            count += 1
    except KeyError:
        return count
    return -1


n2 = counted({}, 3)
#@value n2: includes={3}
#@todo value n2: exactly={3}  (yields inside the generator's own loop have no positions: its raise is taken at every item)


def broken_off(d):
    seen = []
    try:
        for v in failing(d):
            seen.append(v)
            if v == 1:
                break
    except KeyError:
        return "raised"
    return len(seen)


n3 = broken_off({"a": 1})
#@value n3: exactly={1}


def checked(xs):
    for x in xs:
        if x < 0:
            raise ValueError(x)
        yield x


def total(xs):
    acc = []
    try:
        for v in checked(xs):
            acc.append(v)
    except ValueError:
        return sum(acc)
    return -1


t = total([1, 2, -3, 4])
#@value t: includes={3}
#@todo value t: exactly={3}  (as n2: the raise inside the generator's loop has no position)


def comp(d):
    out = []
    try:
        [out.append(v) for v in failing(d)]
    except KeyError:
        return len(out)
    return -1


c = comp({"a": 1})
#@value c: exactly={1}
