# Unknowns of a known type print as `int(?)`, `str(?)`: a parameter's annotation (trusted in the
# function's run for any call), a builtin's result, an operator on operands of known types.
import sys


def describe(n: int, name: str, flag: bool, ratio: float):
    total = n + 1
    #@general total: types={int}
    #@value total: exactly={4}
    label = name.upper()
    #@general label: types={str}
    #@value label: exactly={AB}
    size = len(name)
    #@general size: types={int}
    text = f"{name}={n}"
    #@general text: types={str}
    half = n / 2
    #@general half: types={float}
    neg = not flag
    #@general neg: types={bool}
    same = n == 3
    #@general same: types={bool}
    # (a `float` parameter may be given an int: the product is an int or a float)
    scaled = ratio * 2
    #@general scaled: untyped
    return total, label, size, text, half, neg, same, scaled


def loose(v):
    # (no annotation: `v + 1` runs whatever `v`'s class does)
    w = v + 1
    #@general w: untyped
    #@value w: exactly={3}
    return w


count = len(sys.argv)
#@value count: types={int}
first = str(sys.argv[0])
#@value first: types={str}
width = int(count)
#@value width: types={int}
shown = repr(sys.argv)
#@value shown: types={str}
out = describe(3, "ab", True, 0.5)
w = loose(2)
