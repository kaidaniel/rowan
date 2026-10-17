"""A pydantic model built from data rowan cannot see (`json.loads` of a text from the
environment) holds validated values: a `str` field a str, an `int` field an int, a `float`
field a float (pydantic converts an int given for it), `Optional[str]` a str or None. The
fields of a model built with `model_validate` are the same.

Expectations were written from the program text and pydantic's documented behaviour before
running the engine."""
import json
import os
from typing import Optional

from pydantic import BaseModel


class Summary(BaseModel):
    region: str
    total: float
    n_orders: int
    note: Optional[str] = None


def build(text):
    d = json.loads(text)
    s = Summary(region=d["region"], total=d["total"], n_orders=d["n_orders"], note=d["note"])
    region = s.region
    #@value region: types={str}
    n = s.n_orders
    #@value n: types={int}
    total = s.total
    #@value total: types={float}
    note = s.note
    #@value note: types={str,NoneType}
    v = Summary.model_validate(d)
    vt = v.total
    #@value vt: types={float}
    return s


DATA = '{"region": "EU", "total": 2, "n_orders": 3, "note": null}'
build(os.environ.get("ROWAN_CASE_DATA", DATA))
build(os.environ.get("ROWAN_CASE_DATA", DATA.replace("null", '"hi"')))
