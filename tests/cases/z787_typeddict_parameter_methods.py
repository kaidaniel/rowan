"""A parameter annotated with a TypedDict is a dict at runtime: its methods are dict's, for any
call (a read of `opts.setdefault` never fails), and its declared keys are what it may hold.
(Found by the held-out corpus: pluggy's `HookimplOpts`, whose `opts.setdefault(...)` calls
were reported as failing on every run.)"""
from typing import TypedDict


class Opts(TypedDict, total=False):
    tryfirst: bool
    wrapper: bool


class Point(TypedDict):
    x: int
    y: int


# (a dict's method read has no verdict of its own: `sound`, not `ok` — it must not be one that fails)
def normalize(opts: Opts) -> Opts:
    opts.setdefault("tryfirst", False)  #@ sound setdefault
    opts.setdefault("wrapper", False)  #@ sound setdefault
    first = opts.get("tryfirst")  #@ sound get
    keys = sorted(opts.keys())  #@ sound keys
    return opts


def norm(p: Point) -> int:
    total = p["x"] + p["y"]  #@ ok x ; ok y
    n = len(p.items())  #@ sound items
    return total + n


def uncalled(opts: Opts):
    # (no caller: its run for any call is the only one)
    return opts.get("wrapper")  #@ sound get


o = normalize({"wrapper": True})
r = norm({"x": 1, "y": 2})  #@value r: exactly={5}
