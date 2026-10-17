# Registry decorators past their budget keep the function where the program reads it back:
# appended to a global list, `setdefault` in a global dict, stored in a class's dict attribute
# or in a dict the receiver holds (`@hub.register`), or by a helper the decorator calls. Like
# `G[name] = fn` (z646), none of these gives the function to code that may call it, so the
# skipped registration need not apply what the function may do; it is in the container and
# runs where the program calls it from there. They were each taken to give the function away
# (its code walked: STATE may have changed before any model was built).
#@limits budget=1
STATE = {"built": "none"}
_list = []
_map = {}


class Registry:
    entries = {}


class Hub:
    def __init__(self):
        self.fns = {}

    def register(self, fn):
        name = fn.__name__
        self.fns[name] = fn
        return fn


HUB = Hub()


def _add(name, fn):
    _map[name] = fn


def reg_list(fn):
    name = fn.__name__
    _list.append(fn)
    return fn


def reg_default(fn):
    name = fn.__name__
    _map.setdefault(name, fn)
    return fn


def reg_class(fn):
    name = fn.__name__
    Registry.entries[name] = fn
    return fn


def reg_helper(fn):
    name = fn.__name__
    _add(name, fn)
    return fn


def warm():
    return "w"


# (the first call of each runs, past the budget of 1 statement; the next ones are skipped)
reg_list(warm)
reg_default(warm)
reg_class(warm)
HUB.register(warm)
reg_helper(warm)


@reg_list
def m_a():
    STATE["built"] = "a"
    return "A"


@reg_default
def m_b():
    STATE["built"] = "b"
    return "B"


@reg_class
def m_c():
    STATE["built"] = "c"
    return "C"


@HUB.register
def m_d():
    STATE["built"] = "d"
    return "D"


@reg_helper
def m_e():
    STATE["built"] = "e"
    return "E"


before = STATE["built"]
# (no model ran yet: the registrations only stored them)
#@value before: exactly={'none'}
ra = _list[1]()
#@value ra: includes={'A'}
rb = _map["m_b"]()
#@value rb: includes={'B'}
rc = Registry.entries["m_c"]()
#@value rc: includes={'C'}
rd = HUB.fns["m_d"]()
#@value rd: includes={'D'}
re_ = _map["m_e"]()
#@value re_: includes={'E'}
built = STATE["built"]
#@value built: includes={'e'}
# (the skipped registrations walk none of the models' code: 44 visits before, 35 now)
#@visits <= 40
