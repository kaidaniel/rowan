"""Hypothesis: a parameter annotated with one of the program's dataclasses, NamedTuples or
TypedDicts is, for any call, an instance whose fields (its own and its bases') are there and
hold values of their annotated types, as a parameter annotated with that type would: `int` an
int, `list[str]` a list of str, `Optional[str]` None or a str, another such class an instance
of it (a class that refers to itself: its instance's own fields too). `float` is not claimed
(an int passes for a float). (`d`: types names NoneType too, the oracle checks every value.)

Expectations were written from the program text before running the engine."""
from dataclasses import dataclass, field
from typing import NamedTuple, Optional, TypedDict

from pydantic import BaseModel


@dataclass
class Base:
    ident: int
    label: str


@dataclass
class Item(Base):
    qty: int
    note: Optional[str] = None
    tags: list[str] = field(default_factory=list)


class Point(NamedTuple):
    x: int
    y: float


class Cfg(TypedDict):
    host: str
    port: int


@dataclass
class Order:
    item: Item
    count: int


@dataclass
class Link:
    val: int
    nxt: Optional["Link"] = None


def describe(it: Item, p: Point, cfg: Cfg):
    a = it.ident  #@ ok ident
    #@general a: types={int}
    b = it.label.upper()  #@ ok label
    #@general b: types={str}
    c = it.qty * 2  #@ ok qty
    #@general c: types={int}
    d = it.note
    #@general d: includes={None} types={str,NoneType}
    for t in it.tags:
        e = t
        #@general e: types={str}
    f = p.x
    #@general f: types={int}
    g = p.y
    #@general g: untyped
    h = cfg["port"]  #@ ok port
    #@general h: types={int}
    return a, b, c, d, f, g, h


def total(o: Order):
    q = o.item.qty  #@ ok qty
    #@general q: types={int}
    i = o.item.ident  #@ ok ident
    #@general i: types={int}
    return q * o.count


def walk(n: Link):
    v = n.val  #@ ok val
    #@general v: types={int}
    if n.nxt is not None:
        w = n.nxt.val  #@ ok val
        #@general w: types={int}
        return v + w
    return v


def first_qty(items: list[Item], by_name: dict[str, Point]):
    # (the elements of a container annotated with a class: instances of it)
    for it in items:
        k = it.qty  #@ ok qty
        #@general k: types={int}
        return k + by_name["a"].x  #@ ok x
    return 0


class Person(BaseModel):
    ident: int


class Customer(Person):
    home: str


def uncalled(it: Item, c: Customer):
    # (nothing calls it: its own run for any call reports these reads, its bases' fields too)
    return it.ident, it.qty, c.ident, c.home  #@ ok ident ; ok qty ; ok home


if __name__ == "__main__":
    it = Item(1, "a", 2, None, ["x"])
    describe(it, Point(1, 2.5), {"host": "h", "port": 80})
    describe(Item(2, "b", 3, "n"), Point(0, 1), {"host": "g", "port": 1})
    total(Order(it, 3))
    walk(Link(1, Link(2)))
    walk(Link(3))
    first_qty([it], {"a": Point(1, 2.0)})
