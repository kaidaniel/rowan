"""`match` on a value that is a literal on some paths and a container or instance on others:
`case None` / `case 3` / `case {"k": v}` may match the literal part (or the dict), so neither the
case nor the fall-through is unreachable."""
import sys


class Box:
    v = 1


def classify(x):
    match x:
        case None:
            kind = "none"
        case 3:
            kind = "three"
        case {"k": v}:
            kind = "map"
        case _:
            kind = "other"
    return kind


a = classify(None if len(sys.argv) > 1 else [])
#@value a: exactly={"none", "other"}
b = classify(3 if len(sys.argv) > 1 else Box())
#@value b: exactly={"three", "other"}
c = classify({"k": 1} if len(sys.argv) > 1 else "s")
#@value c: exactly={"map", "other"}
d = classify(True if len(sys.argv) > 1 else [])
#@value d: exactly={"other"}


def one(x):
    match x:
        case 1:
            return "one"
    return "not"


e = one(True)
#@value e: exactly={"one"}
