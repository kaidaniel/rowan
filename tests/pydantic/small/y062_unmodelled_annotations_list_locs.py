# A field whose annotation rowan does not model (a fixed-length tuple, a union of scalars)
# may convert the value or reject it: `tuple[int, int]` makes a tuple of a list, and
# `Union[int, str]` rejects None. An error in an item of a list of models is located at
# (field, position, item field). Before, such fields kept the value as given and never failed,
# and the position was left out of the location.
# Expectations were written from the program text and pydantic's documented behaviour
# before running the engine.
from typing import Union

from pydantic import BaseModel, ValidationError


class Pair(BaseModel):
    t: tuple[int, int] = (0, 0)
    u: Union[int, str] = 0


class Item(BaseModel):
    a: int


class Order(BaseModel):
    items: list[Item] = []


p = Pair(t=[1, 2])
is_tuple = isinstance(p.t, tuple)                        #@value is_tuple: includes={True}
try:
    Pair(u=None)
    u = "ok"
except ValidationError:
    u = "invalid"
vu = u                                                   #@value vu: includes={"invalid"}
try:
    Order(items=[{"a": 1}, {"a": "x"}])
    where = "ok"
except ValidationError as e:
    where = ".".join(str(x) for x in e.errors()[0]["loc"])
vw = where                                               #@value vw: exactly={"items.1.a"}
