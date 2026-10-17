"""Hypothesis: a module-level dict that functions only copy (`{**DEFAULTS, …}`, `dict(DEFAULTS)`,
`[*ITEMS]`) is not changed by them: the copy is a new container holding DEFAULTS' values, so
giving the copy away (returning it) gives away those values, not DEFAULTS. For any call,
DEFAULTS still has its keys and values (`retries` is 3, not `3 | ?`). A copy's values are
DEFAULTS' own, so a mutable one changed through the copy does change (LIMITS' list).

Expectations were written from the program text before running the engine."""
import json

DEFAULTS = {"retries": 3, "timeout": 10}
ITEMS = ["a", "b"]
LIMITS = {"sizes": [1]}


def load(text):
    cfg = {**DEFAULTS, **json.loads(text)}
    t = cfg["timeout"]
    #@general t: includes={10}
    return cfg


def copied():
    return dict(DEFAULTS)


def listed():
    return [*ITEMS, "c"]


def retries():
    r = DEFAULTS["retries"]
    #@general r: exactly={3}
    n = len(ITEMS)
    #@general n: exactly={2}
    return r, n


def grow():
    c = {**LIMITS}
    c["sizes"].append(2)
    return c


def sizes():
    k = len(LIMITS["sizes"])
    #@general k: includes={1, 2}
    return k


if __name__ == "__main__":
    load('{"timeout": 5}')
    retries()
