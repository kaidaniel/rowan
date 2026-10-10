"""Straight-line generators (yields of reads, no calls) are summarised like functions returning
their items, then cloned per call: each call has its own items and one-shot cursor."""
import sys


class Style:
    def __init__(self, color, bold):
        self.color = color
        self.bold = bold

    def __rich_repr__(self):
        yield "color", self.color
        if self.bold:
            yield "bold", True
        yield "kind", "style"


def fields(d):
    yield d["a"]
    x = d["b"]
    yield x
    yield


red = Style("red", True)
blue = Style("blue", False)
r = list(red.__rich_repr__())
rt = tuple(r)
#@value rt: exactly={(("color", "red"), ("bold", True), ("kind", "style"))}
nr = len(r)
#@value nr: exactly={3}
b = list(blue.__rich_repr__())
nb = len(b)
#@value nb: exactly={2}
b1 = b[1]
#@value b1: exactly={("kind", "style")}

g = red.__rich_repr__()
first = next(g)
#@value first: exactly={("color", "red")}
rest = list(g)
nrest = len(rest)
#@value nrest: exactly={2}

keys = tuple([k for k, v in blue.__rich_repr__()])
#@value keys: exactly={("color", "kind")}

either = Style("green", len(sys.argv) > 2)
ne = len(list(either.__rich_repr__()))
# (lengths 2 or 3: the alternatives of the joined paths, shifted by the last yield)
#@value ne: exactly={2, 3}

f = tuple(fields({"a": 1, "b": 2}))
#@value f: exactly={(1, 2, None)}
pairs = tuple(zip(fields({"a": "x", "b": "y"}), [10, 20]))
#@value pairs: exactly={(("x", 10), ("y", 20))}


def needs(row):
    return list(fields(row))
#@schema param needs.row: always={a, b}
