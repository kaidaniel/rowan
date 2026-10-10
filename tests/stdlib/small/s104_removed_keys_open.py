"""A key removed from an open record (os.environ, json.loads of input) by pop / del is absent:
`in`, `.get`, `pop` with a default decide it, also inside functions that read os.environ (decided per
call site: the driver popped the variable before the first call and set it before the second)."""
import json
import os
import sys


def overrides(prefix="APP_"):
    out = []
    if os.environ.get(prefix + "TITLE") is not None:
        out.append("title")
    if prefix + "WIDTH" in os.environ:
        out.append("width")
    return out


if __name__ == "__main__":
    os.environ.pop("APP_TITLE", None)
    os.environ.pop("APP_WIDTH", None)
    t = os.environ.get("APP_TITLE")              #@value t: exactly={None}
    w = "APP_WIDTH" in os.environ                #@value w: exactly={False}
    o1 = len(overrides())                        #@value o1: exactly={0}
    os.environ["APP_WIDTH"] = "30"
    o2 = len(overrides())                        #@value o2: exactly={1}
    d = json.loads(os.environ.get("S104_D", '{"a": 1, "b": 2, "c": 3}'))  # (was sys.argv[1]: no argv in the oracle run)
    d.pop("a", None)
    del d["b"]
    a = d.get("a")                               #@value a: exactly={None}
    ina = "a" in d                               #@value ina: exactly={False}
    inb = "b" in d                               #@value inb: exactly={False}
    p = d.pop("b", 0)                            #@value p: exactly={0}
    c = d.get("c")                               #@value c: unknown
