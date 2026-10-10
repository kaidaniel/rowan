"""Hypothesis: a loop over a closed dict whose keys are known but some only maybe there runs
each sure key and maybe each other one: a merge keeps the sure keys' values exact (no unknown
key overwrites everything), the maybe keys become maybe keys of the result."""
import os


def layer(environ):
    out = {"name": "base"}
    if "X_PORT" in environ:
        out["port"] = environ["X_PORT"]
    return out


def merge(base, over):
    out = dict(base)
    for key, val in over.items():
        out[key] = val
    return out


cfg = merge({"name": "app", "debug": False}, layer(os.environ))
#@row cfg: present={name,debug,port} definite={name,debug} closed
name = cfg["name"]                               #@value name: exactly={"base"}
dbg = cfg["debug"]                               #@value dbg: exactly={False}
