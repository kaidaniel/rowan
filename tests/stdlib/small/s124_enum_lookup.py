"""Hypothesis: Enum lookups by value `Color(v)` and by name `Color[n]` fail differently (a value
lookup raises ValueError, a name lookup KeyError) and helpers catching each give exact members
or None; an alias (a second name with the same value) resolves to the first member and is not
iterated; Flag members combine with |= over Perm[letter] and `m in p` tests bits. A code from
the environment parses to None or one of the two canonical members."""
from enum import Enum, Flag, auto
import os


class Color(Enum):
    RED = "r"
    GREEN = "g"
    CRIMSON = "r"


class Perm(Flag):
    R = auto()
    W = auto()
    X = auto()


def by_value(v):
    try:
        return Color(v)
    except ValueError:
        return None


def by_name(n):
    try:
        return Color[n]
    except KeyError:
        return None


def parse(code):
    c = by_value(code)
    return c if c is not None else by_name(code)


def grant(letters):
    p = Perm(0)
    for ch in letters:
        p |= Perm[ch]
    return p


def describe(p):
    return {m.name: (m in p) for m in Perm}


if __name__ == "__main__":
    a = parse("g").name                          #@value a: exactly={"GREEN"}
    b = parse("RED") is Color.RED                #@value b: exactly={True}
    al = parse("CRIMSON").name                   #@value al: exactly={"RED"}
    c = parse("blue")                            #@value c: exactly={None}
    n = len(list(Color))                         #@value n: exactly={2}
    p = grant("RX")
    pv = p.value                                 #@value pv: exactly={5}
    d = describe(p)
    #@row d: present={R,W,X} definite={R,W,X} closed
    dw = d["W"]                                  #@value dw: exactly={False}
    dx = d["X"]                                  #@value dx: exactly={True}
    ok = parse(os.environ.get("S124_C", "r")) in (None, Color.RED, Color.GREEN)
    #@value ok: exactly={True}
