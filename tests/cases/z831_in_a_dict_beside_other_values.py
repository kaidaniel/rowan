"""`"k" in x` where x is a dict that has "k" on some paths and a str or a list on others: the
test may be false (the str or the list lacks it), so the else branch runs."""
import sys

many = len(sys.argv) > 1


def where(x):
    if "k" in x:
        return "in"
    else:
        return "out"


a = where({"k": 1} if many else "")
#@value a: exactly={"in", "out"}
b = where({"k": 1} if many else [])
#@value b: exactly={"in", "out"}
c = where({"k": 1})
#@value c: exactly={"in"}
