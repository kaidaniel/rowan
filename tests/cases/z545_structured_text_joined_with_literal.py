"""A JSON text rowan knows the parts of (`json.dumps` of a dict with an unknown value) joined
with another text (`"{}"`) is either: reading "a" from its parse may fail. The join kept the
JSON text and dropped the other one, so the read was proven.

Expectations were written from the program text before running the engine."""
import json
import os

n = int(os.environ.get("Z545_N", "3"))
flag = os.environ.get("Z545_FLAG") == "1"


def parse(c):
    t = json.dumps({"a": n}) if c else "{}"
    d = json.loads(t)
    return d["a"]                        #@ input a
    #@todo possibly a (the joined text is any text to rowan: its parse is an input's; proven was wrong)


def parse_none(c):
    t = None if c else json.dumps({"a": n})
    if t is None:
        return "none"
    return "text"


try:
    v = parse(flag)
except KeyError:
    v = "missing"
r = parse_none(flag)                     #@value r: exactly={"none", "text"}
