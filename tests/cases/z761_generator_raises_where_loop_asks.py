# A generator raises where its consumer asks for the item it fails to give: a loop over it has
# run its body for the items before (a handler sees what the body did), and nothing after the
# loop runs on that path; `list` of one that surely raises raises there, nothing after it runs.
def failing(d):
    yield d["a"]
    yield d["b"]


def loop(d):
    seen = []
    try:
        for v in failing(d):
            seen.append(v)
        ok = True
    except KeyError:
        ok = False
    return ok, len(seen)


ok, n = loop({"a": 1})
#@value ok: includes={False}
#@value n: includes={1}
#@todo value ok: exactly={False}  (a generator's raises are taken where it runs whole, at the loop's start, and its loop's end is not known to be unreached)
#@todo value n: exactly={1}  (its handler's state is the loop's start's: the normal end's joined in keeps it sound)


def collect(d):
    try:
        out = list(failing(d))
    except KeyError:
        out = None
    return out


r = collect({"a": 1})
#@value r: exactly={None}
seen2 = []
try:
    for v in failing({"a": 5, "b": 6}):
        seen2.append(v)
    done = True
except KeyError:
    done = False
d2 = done
#@value d2: exactly={True}
m = len(seen2)
#@value m: exactly={2}
