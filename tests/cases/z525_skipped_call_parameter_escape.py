"""A call past its budget changes a parameter's default only where its code may change the
parameter's value: read, compared, a method that keeps nothing called on it, it changes
nothing; iterated, only what it holds may change (an iterator advances); stored into the
instance (`self.box = box`), passed on, returned or changed in place, it may change. A
global bound to a local (`t = TABLE`) changes only where the local does. (Each function's
second call is past the budget.)

Expectations were written from the program text before running the engine."""
#@limits budget=1


class NoChange:
    pass


NO_CHANGE = NoChange()
OPTS = {"a": 1}
SEEN = [{"k": 1}]
LOG = []
BOX = {"w": 1}
CACHE = {"x": 1}
NAMES = set()
WIDTHS = [(0, 1), (2, 3)]
STEPS = iter([1, 2, 3])


def update(width=NO_CHANGE, opts=OPTS, seen=SEEN, log=LOG):
    a = 1
    if isinstance(width, NoChange):
        width = 0
    x = opts.get("a")
    for s in seen:
        s["k"] = 2
    log.append(width)
    return x


update(1)
update(2)
o = OPTS["a"]  #@ ok a
#@value o: exactly={1}
n = len(SEEN)  #@value n: exactly={1}
first = SEEN[0]
kk = first["k"]  #@ sound k
#@value kk: includes={2}
m = len(LOG)  #@value m: includes={2}


class Table:
    def __init__(self, box=BOX):
        a = 1
        self.box = box


def make(change):
    a = 1
    t = Table()
    if change:
        t.box["w"] = 2
    return t


make(False)
make(True)
w = BOX["w"]  #@ sound w
#@value w: includes={2}


def look(k, _get=CACHE.get, _add=NAMES.add):
    a = 1
    _add(k)
    return _get(k)


look("x")
look("y")
cx = CACHE["x"]  #@ ok x
#@value cx: exactly={1}
seen_y = "y" in NAMES  #@value seen_y: includes={True}


def width(i):
    a = 1
    table = WIDTHS
    return table[i][1]


width(0)
width(1)
nw = len(WIDTHS)  #@value nw: exactly={2}


def step():
    a = 1
    for x in STEPS:
        return x


step()
step()
third = next(STEPS, None)  #@value third: includes={3}
