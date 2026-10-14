"""A builtin given an unknown (`len`, `str.join`) may raise a TypeError; a handler that may
catch it gets an exception object of its class. One object per class serves every such
point: an object per call site and context (and its args tuple) at every `len` of an unknown
inside a running `try` made rich's objects grow by half."""
import json
import sys


def width(x):
    return len(x)


def words(parts):
    return " ".join(parts)


def layout(row):
    return width(row["a"]) + width(row["b"]) + width(row["c"]) + width(row["d"])


def render(rows):
    out = []
    try:
        for row in rows:
            out.append(layout(row))
            out.append(words(row["e"]))
    except Exception as e:
        out.append(type(e).__name__)
    return out


rows = json.loads(sys.argv[1]) if len(sys.argv) > 1 else [{"a": "x", "b": "yy", "c": [1], "d": {}, "e": ["p", "q"]}]
r1 = render(rows)
r2 = render(rows[:1])
r3 = render([{"a": 1, "b": "", "c": "", "d": "", "e": []}])
r4 = render([{"a": "", "b": "", "c": "", "d": "", "e": [1]}])
first = r3[0]
#@value first: includes={"TypeError"}
#@objects <= 150
