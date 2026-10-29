"""`obj.__dict__` and `vars(obj)` are one dict, the object's attributes themselves: a change
through either is a change of the attributes (`pop`, `setdefault`, `clear`, `update`, `|=`,
`del`), a change of the attributes shows through a dict taken earlier, and reading it
(iterating, `len`, `in`, `keys()` / `values()` / `items()`, `copy()`, `copy.copy`, `dict(...)`,
`**`, `isinstance`) gives the attribute names and values. (Before, `vars(obj)` was a copy whose
changes other than item stores were lost, and iterating `obj.__dict__` gave the object instead
of its names.)

Expectations were written from the program text before running the engine."""
import copy


class P:
    def __init__(self, k):
        self.k = k
        self.j = "s"


def kw(**named):
    return named


def live(p):
    d = vars(p)
    p.k = 5
    a1 = d["k"]
    #@value a1: exactly={5}
    d["k"] = 6
    a2 = p.k
    #@value a2: exactly={6}
    same = d is p.__dict__
    #@value same: exactly={True}
    d |= {"k": 8}
    a3 = p.k
    #@value a3: exactly={8}


def reads(p):
    for name in p.__dict__:
        n1 = name
        #@value n1: exactly={"k", "j"}
    for name in vars(p).keys():
        n2 = name
        #@value n2: exactly={"k", "j"}
    for value in vars(p).values():
        v1 = value
        #@value v1: exactly={1, "s"}
    for name, value in p.__dict__.items():
        n3 = name
        #@value n3: exactly={"k", "j"}
    size = len(p.__dict__)
    #@value size: exactly={2}
    has = "j" in vars(p)
    #@value has: exactly={True}
    cp = p.__dict__.copy()
    cp["new"] = 1
    #@row cp: present={k, j, new} definite={k, j, new} closed
    gained = hasattr(p, "new")
    #@value gained: exactly={False}
    made = dict(vars(p))
    #@row made: present={k, j} definite={k, j} closed
    spread = kw(**p.__dict__)
    #@row spread: present={k, j} definite={k, j} closed
    is_dict = isinstance(vars(p), dict) and type(p.__dict__) is dict
    #@value is_dict: exactly={True}
    shallow = copy.copy(p.__dict__)
    shallow["k"] = 0
    still = p.k
    #@value still: exactly={1}


def changes(p):
    got = p.__dict__.pop("k")
    #@value got: exactly={1}
    try:
        p.k
        r1 = "kept"
    except AttributeError:
        r1 = "gone"
    #@value r1: exactly={"gone"}
    dflt = vars(p).pop("zz", None)
    #@value dflt: exactly={None}
    vars(p).setdefault("m", 3)
    m = p.m
    #@value m: exactly={3}
    vars(p).setdefault("j", 4)
    j = p.j
    #@value j: exactly={"s"}
    vars(p).update(k=7)
    k = p.k
    #@value k: exactly={7}
    del vars(p)["m"]
    #@row p: present={j, k} definite={j, k} closed
    p.__dict__.clear()
    #@row p: present={} definite={} closed


live(P(1))
reads(P(1))
changes(P(1))
