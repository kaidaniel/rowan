"""A read that surely fails (a key a dict never has) raises: no run goes past it, and the name
it would bind holds nothing there (rowan goes on past it to report what follows). A line's
value is what the runs that reach it give. Before, the path past the failing read added
"anything" there: `v` was `1 | ?`, while the run for any call said 1.

Expectations were written from the program text before running the engine."""
import sys


def lookup(flag):
    d = {}
    if flag:
        d["a"] = 1
    v = d["a"]
    #@value v: exactly={1}
    #@general v: exactly={1}
    return v


x = lookup(True)
#@value x: exactly={1}
# (no handler around: the read's KeyError would end the program)
y = lookup(False) if len(sys.argv) > 5 else None
