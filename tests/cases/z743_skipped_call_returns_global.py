# A call that does not run (every call, at budget 0) may give back what its code reads: a
# function a global registry holds (`REG.get(name, fn)`), a global itself, a global's part
# under a literal key, or what a function it calls by name gives back. Stores into the result
# are stores into those, the result called runs them. Where nothing like that happened, a
# function's attribute never stored is surely not there (hasattr is False).
#@limits budget=0
import sys

n = len(sys.argv)
REG = {}
CONFIG = {"a": 1}
SETTINGS = {"db": {"host": "x"}}


def f0():
    return 0


REG["f0"] = f0


def lookup(fn):
    return REG.get("f0", fn)


def g():
    return 1


h = lookup(g)
h.tag = 5
t = getattr(f0, "tag", "-")
#@value t: includes={5}
ok = hasattr(f0, "tag")
#@value ok: includes={True}
r = h()
#@value r: includes={0}


def pick():
    return CONFIG


c = pick()
c["b"] = 2
cb = CONFIG.get("b")
#@value cb: includes={2}


def part():
    return SETTINGS["db"]


d = part()
d["port"] = 1
port = SETTINGS["db"].get("port")
#@value port: includes={1}


def via():
    return lookup(None)


v = via()
v.seen = True
seen = getattr(f0, "seen", False)
#@value seen: includes={True}


def plain():
    return 3


plain.color = "red"
if n > 3:
    plain.size = 1
has_color = hasattr(plain, "color")
#@value has_color: exactly={True}
has_shape = hasattr(plain, "shape")
#@value has_shape: exactly={False}
has_size = hasattr(plain, "size")
#@value has_size: exactly={True, False}
