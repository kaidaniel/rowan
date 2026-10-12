"""A summary past the size bound (here 4 entries, `#@limits summary=4`) is not
instantiated: a call is an unknown call on its arguments, and what the function changed of
objects from outside (a global registry) is changed in unknown ways there. Instantiating
summaries of hundreds of thousands of entries per call made rich take hours."""
#@limits summary=4
import sys

REGISTRY = {"base": 1}


def register(cfg, name):
    cfg["seen"] = True
    REGISTRY[name] = cfg["value"]
    a = {"x": 1}
    b = {"y": 2}
    return [a, b]


cfg = {"value": 3}
out = register(cfg, "extra")
seen = cfg.get("seen")
#@value seen: includes={True}
got = REGISTRY.get("extra")
#@value got: includes={3}
base = REGISTRY["base"]
#@value base: includes={1}
