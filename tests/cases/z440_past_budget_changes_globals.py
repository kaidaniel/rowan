# A call past its function's budget runs as unknown code; that code may still change what
# the function reaches beyond its arguments: module globals it changes or rebinds, closure
# cells it rebinds, class attributes it changes through `self`. (Each function's second call
# is past the budget; the code it calls, by name or as an argument, counts too.)
#@limits budget=2

STATE = {"n": 0, "tag": "x"}
LOG = []
COUNT = 0


def drop():
    a = 1
    b = 2
    if "tag" in STATE:
        del STATE["tag"]


def note(msg):
    a = 1
    b = 2
    LOG.append(msg)


def bump():
    global COUNT
    a = 1
    COUNT = COUNT + 1


def make():
    k = 0

    def inc():
        nonlocal k
        a = 1
        b = 2
        k += 1

    def get():
        return k

    return inc, get


class Reg:
    items = []

    def add(self, x):
        a = 1
        b = 2
        self.items.append(x)


drop()
STATE["tag"] = "again"
drop()
t = STATE.get("tag")  #@value t: includes={None}
note("a")
note("b")
n = len(LOG)  #@value n: includes={2}
bump()
bump()
c = COUNT  #@value c: includes={2}
inc, get = make()
inc()
inc()
kv = get()  #@value kv: includes={2}
r1 = Reg()
r1.add(1)
r1.add(2)
m = len(Reg.items)  #@value m: includes={2}

REGISTRY = {}


def register(name):
    REGISTRY[name] = True


def setup(name):
    a = 1
    b = 2
    register(name)


setup("x")
setup("y")
ry = REGISTRY.get("y")  #@value ry: includes={True}

HITS = []


def on_hit():
    HITS.append(1)


def run(cb):
    a = 1
    b = 2
    cb()


run(on_hit)
run(on_hit)
h = len(HITS)  #@value h: includes={2}
