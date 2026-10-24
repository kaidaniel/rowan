"""`k in x` where x is None or a number raises TypeError, and `3 in "abc"` too; `"b" in "abc"`
does not."""
import sys


def has(k, x):
    try:
        return "yes" if k in x else "no"
    except TypeError:
        return "error"


a = has("k", None)
#@value a: exactly={"error"}
b = has("k", 3 if len(sys.argv) > 1 else None)
#@value b: exactly={"error"}
c = has(3, "abc")
#@value c: exactly={"error"}
d = has("b", "abc")
#@value d: exactly={"yes"}
e = has("k", None if len(sys.argv) > 1 else {"k": 1})
#@value e: exactly={"error", "yes"}
