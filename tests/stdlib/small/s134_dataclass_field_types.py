# The program builds dataclass, NamedTuple and TypedDict instances from data it cannot see
# (`json.loads` of a text from the environment). Each field holds what its annotation says
# (rowan's soundness is relative to annotations): an unknown given for a `str` field is a
# str, for an `int` field an int, for `Optional[str]` a str or None. `float` is not trusted
# for an unknown: these classes convert nothing, and an int passes for a float. Literals
# keep their values. Expectations written from the program text before running the engine.
import json
import os
from dataclasses import dataclass
from typing import NamedTuple, Optional, TypedDict


@dataclass
class Summary:
    region: str
    total: float
    n_orders: int
    note: Optional[str] = None


class Point(NamedTuple):
    x: int
    label: str


class Row(TypedDict):
    id: int
    name: str


def build(text):
    d = json.loads(text)
    s = Summary(d["region"], d["total"], d["n"], d["note"])
    region = s.region
    #@value region: types={str}
    n = s.n_orders
    #@value n: types={int}
    total = s.total
    #@value total: untyped
    note = s.note
    #@value note: types={str,NoneType}
    p = Point(d["x"], label=d["label"])
    px = p.x
    #@value px: types={int}
    r = Row(id=d["id"], name=d["name"])
    rid = r["id"]
    #@value rid: types={int}
    lit = Summary("EU", 1, 2)
    t = lit.total
    #@value t: exactly={1}
    return s


DATA = '{"region": "EU", "total": 2, "n": 3, "note": null, "x": 1, "label": "a", "id": 7, "name": "b"}'
build(os.environ.get("ROWAN_CASE_DATA", DATA))
build(os.environ.get("ROWAN_CASE_DATA", DATA.replace("null", '"hi"')))
