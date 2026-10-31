"""`functools.update_wrapper(wrapper, wrapped)` in a followed package's code (`cmdlib`) sets the
wrapper's attributes from the wrapped function's and changes nothing else: the program's
function it is given does not run there, its defaults (`opts`) and what its code changes
(`CALLS`) stay as they are. CPython: n == 0, depth == 1, then 2 calls, e == 3.

Expectations were written from the program text before running the engine."""
#@argv --follow cmdlib --site-packages site
import cmdlib

SETTINGS = {"mode": "fast"}
CALLS = []


@cmdlib.command
def show(opts={"depth": 1}):
    CALLS.append(SETTINGS["mode"])
    return opts["depth"]


def helper(x):
    return x * 2


n = len(CALLS)                               #@value n: exactly={0}
mode = SETTINGS["mode"]                      #@value mode: exactly={"fast"}
depth = show()                               #@value depth: exactly={1}
e = cmdlib.COMMANDS["show"](opts={"depth": 3})  #@value e: includes={3}
m = len(CALLS)                               #@value m: includes={2}
g = cmdlib.wrapped(helper)
r = g(2)                                     #@value r: includes={4}
