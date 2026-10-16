"""A dict iterates in insertion order. A key that `dict.fromkeys` of a list of unknown length
may have put in comes before the keys stored after it: `"a" in d` says it is there, not that
it came last. rowan appended the key the test found to the order (`b, a`), and a join kept the
longer order of two paths when the shorter one held the key at another place.

Expectations were written from the program text before running the engine."""
import os

flag = os.environ.get("Z546_FLAG") == "1"


def found(flag):
    xs = [x for x in ["a"] if flag]
    d = dict.fromkeys(xs, 0)
    d["b"] = 1
    if "a" in d:
        return ",".join(d)
    return "none"


def filled(flag):
    xs = [x for x in ["a"] if flag]
    d = dict.fromkeys(xs, 0)
    d["b"] = 1
    if "a" not in d:
        d["a"] = 2
    return ",".join(d)


r1 = found(flag)                         #@value r1: includes={"a,b"}
r2 = filled(flag)                        #@value r2: includes={"a,b", "b,a"}
r3 = found(True)                         #@value r3: includes={"a,b"}
