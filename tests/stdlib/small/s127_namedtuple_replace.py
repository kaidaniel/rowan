"""Hypothesis: typing.NamedTuple instances are immutable: _replace returns a new tuple (the
original unchanged) and rejects unknown field names (TypeError on 3.14, ValueError before), _asdict gives a dict in
field order with defaults filled in, _field_defaults holds only the fields with defaults, and
assigning an attribute raises AttributeError. A point from the environment needs only `x`."""
from typing import NamedTuple
import json
import os


class Point(NamedTuple):
    x: int
    y: int = 0
    label: str = "p"


def shift(p, dx):
    return p._replace(x=p.x + dx)


def to_row(p):
    d = p._asdict()
    d["sum"] = d["x"] + d["y"]
    return d


def try_set(p):
    try:
        p.x = 9
        return "set"
    except AttributeError:
        return "frozen"


def bad_replace(p):
    try:
        p._replace(z=1)
        return True
    except (TypeError, ValueError):
        return False


if __name__ == "__main__":
    p = Point(1)
    q = shift(p, 4)
    #@row q: present={x,y,label} definite={x,y,label} closed
    px = p.x                                     #@value px: exactly={1}
    qx = q.x                                     #@value qx: exactly={5}
    ql = q.label                                 #@value ql: exactly={"p"}
    r = to_row(q)
    #@row r: present={x,y,label,sum} definite={x,y,label,sum} closed
    rs = r["sum"]                                #@value rs: exactly={5}
    first = list(r)[0]                           #@value first: exactly={"x"}
    ts = try_set(p)                              #@value ts: exactly={"frozen"}
    br = bad_replace(p)                          #@value br: exactly={False}
    fd = Point._field_defaults
    #@row fd: present={y,label} definite={y,label} absent={x} closed
    _, b, _ = Point(2, 3)                        #@value b: exactly={3}
    d = json.loads(os.environ.get("S127_P", '{"x": 3, "y": 4}'))
    #@schema json.loads@L58: always={x} only={x}
    r2 = to_row(Point(d["x"], d.get("y", 0)))
    #@row r2: present={x,y,label,sum} definite={x,y,label,sum} closed
