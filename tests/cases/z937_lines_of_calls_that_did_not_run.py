"""A call past its function's budget does not run; a line inside that function then holds what
any call of it may give there, the calls rowan did not run among them. With `budget=0` a
function's first call runs and its later ones do not: before, a line held the first call's
values alone (`hit` was True, the second call's False left out; `before` was "new").

Expectations were written from the program text before running the engine."""
#@limits budget=0


def contains(codes, code):
    hit = code in codes
    #@value hit: includes={True, False}
    return hit


class Registry:
    def __init__(self):
        self.names = {}

    def seen(self, name):
        before = self.names.get(name, "new")
        #@value before: includes={"new", "old"}
        self.names[name] = "old"
        return before


class Letters:
    def __init__(self):
        self.left = ["a", "b"]

    def __iter__(self):
        return self

    def __next__(self):
        if not self.left:
            raise StopIteration
        x = self.left.pop(0)
        #@value x: includes={"a", "b"}
        return x


def guard(users):
    def check(token):
        ok = token in users
        #@value ok: includes={True, False}
        return ok

    return check


class Track:
    def __init__(self):
        self.stage = "start"

    def __enter__(self):
        return self

    def __exit__(self, et, ev, tb):
        seen = self.stage
        #@value seen: includes={"reading", "done"}
        return True


def managed(row):
    with Track() as t:
        t.stage = "reading"
        n = row["n"]
        t.stage = "done"


contains({"A", "B"}, "A")
contains({"A", "B"}, "Z")
r = Registry()
r.seen("a")
r.seen("a")
letters = [y for y in Letters()]
check = guard({"t1"})
check("t1")
check("zz")
managed({"n": 1})
managed({})
