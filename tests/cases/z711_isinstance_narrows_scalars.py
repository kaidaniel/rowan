# `isinstance` decides and narrows literals and typed unknowns by their type: `None | int(?)`
# is `int(?)` where `isinstance(n, int)` holds and None where it does not; `"a" | 3` splits
# by type. An `int(?)` may be a bool (True joined with an int), so `isinstance(k, bool)` is
# not decided for it, nor is an annotated `str` against the program's classes (a `StrEnum`'s
# member is a str).
import enum
import os


n = None
if os.environ.get("NO_N") is None:
    n = int(os.environ.get("N", "4"))
if isinstance(n, int):
    a = n                                           #@value a: shows={4} types={int}
else:
    b = n                                           #@value b: exactly={None}

x = "a" if os.environ.get("X") else 3
if isinstance(x, str):
    s = x                                           #@value s: exactly={"a"}
else:
    i = x                                           #@value i: exactly={3}
both = isinstance(x, (str, int))                    #@value both: exactly={True}

t = str(os.environ.get("T", "z"))
is_str = isinstance(t, str)                         #@value is_str: exactly={True}
is_int = isinstance(t, int)                         #@value is_int: exactly={False}

k = int(os.environ.get("K", "1"))
if os.environ.get("B"):
    k = True
is_b = isinstance(k, bool)                          #@value is_b: includes={False, True}
is_i = isinstance(k, int)                           #@value is_i: exactly={True}
kl = [k] if os.environ.get("L") else k
is_bl = isinstance(kl, bool)                        #@value is_bl: includes={False, True}


def g(v: int | None, w: str):
    if isinstance(v, int):
        got = v                                     #@general got: types={int}
    else:
        got = v                                     #@general got: exactly={None}
    if isinstance(w, bytes):
        return None
    return w                                        #@general w: types={str}


g(n, t)


class Color(enum.StrEnum):
    RED = "red"


def paint(c: str):
    named = isinstance(c, Color)                    #@general named: includes={True, False}
    return named


painted = paint(Color.RED)                          #@value painted: exactly={True}
