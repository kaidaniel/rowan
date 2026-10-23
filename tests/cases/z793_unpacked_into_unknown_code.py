"""`f(**d)` and `f(*xs)` give f d's values and xs's items in a new dict / tuple, not d or xs:
code rowan does not see, called so, may change what they hold, but not d's keys or xs's length.
(A dict given as `**dd` to unknown code was taken to have gained any key, so every later
`Conv(**dd)` saw each defaulted parameter as possibly anything: the activation looked up by
that name was any of the table's classes, each one constructed: timm's layers, 3x the work.)"""
import textwrap


class A1:
    tag = "a1"

    def __init__(self, inplace=False):
        self.inplace = inplace


class A2:
    tag = "a2"

    def __init__(self, inplace=False):
        self.inplace = inplace


class A3:
    tag = "a3"

    def __init__(self, inplace=False):
        self.inplace = inplace


class A4:
    tag = "a4"

    def __init__(self, inplace=False):
        self.inplace = inplace


ACTS = {"a1": A1, "a2": A2, "a3": A3, "a4": A4}


def get_act(name="a1"):
    if not isinstance(name, str):
        return name
    return ACTS[name.lower()]


class Conv:
    def __init__(self, width, act="a1", inplace=False):
        self.width = width
        self.act = get_act(act)(inplace=inplace)


def block(width, norm):
    dd = {"width": width}
    n = norm(**dd)
    c1 = Conv(**dd)
    c2 = Conv(**dd)
    c3 = Conv(**dd)
    t = c3.act.tag  #@ ok tag
    #@general t: exactly={'a1'}
    ip = c3.act.inplace
    #@general ip: exactly={False}
    return c3


def given_values(norm):
    # the values a `**` mapping holds are given to the code: they may change
    inner = {"k": 1}
    kw2 = {"placeholder": inner}
    norm(**kw2)
    m = inner["k"]  #@ sound k
    #@general m: includes={1} untyped
    held = kw2["placeholder"]  #@ ok placeholder
    return m


def given_items(norm):
    # `*parts` gives the items, not the list
    parts = ["  hello"]
    norm(*parts)
    n = len(parts)
    #@general n: exactly={1}
    return n


b = block(10, lambda **kw: kw)
given_values(lambda **kw: kw)
given_items(lambda *a: a)
# (body runs: the module's calls 15, entry runs 12, block's run for any call 9, Conv.__init__'s
# 5 (its `act` is open), and the general runs the checks ask for again; `dd` taken for open
# made each `Conv(**dd)` construct all four classes: 18 more)
#@tabulations <= 70
