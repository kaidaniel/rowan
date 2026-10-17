"""A parameter annotated with a model is, for any call, a validated instance: its fields (its
own and its bases') are there and hold what validation gives for their annotations: `int` an
int, `float` a float (validation converts an int), `str` a str, `list[str]` a list of str,
`Literal[...]` one of its values, `Optional[M]` None or an instance of M, a nested model an
instance of it.

Expectations were written from the program text before running the engine."""
from typing import Literal, Optional

from pydantic import BaseModel


class Address(BaseModel):
    city: str
    zip: str = "00000"


class Person(BaseModel):
    ident: int
    name: str


class Customer(Person):
    tier: Literal["free", "pro"] = "free"
    score: float = 0.0
    tags: list[str] = []
    address: Optional[Address] = None
    home: Address


def handle(c: Customer):
    a = c.ident  #@ ok ident
    #@general a: types={int}
    b = c.name.title()  #@ ok name
    #@general b: types={str}
    t = c.tier
    #@general t: exactly={"free", "pro"}
    s = c.score
    #@general s: types={float}
    for tag in c.tags:
        g = tag
        #@general g: types={str}
    z = c.home.zip  #@ ok zip
    #@general z: types={str}
    if c.address is not None:
        city = c.address.city  #@ ok city
        #@general city: types={str}
    return a, b, t, s, z


class Book(BaseModel):
    owners: list[Customer] = []


def owner_names(b: Book):
    # (a field's elements annotated with a model: instances of it)
    for o in b.owners:
        n = o.name  #@ ok name
        #@general n: types={str}
    return len(b.owners)


if __name__ == "__main__":
    handle(Customer(ident=1, name="ann", home={"city": "Oslo"}))
    owner_names(Book(owners=[{"ident": 3, "name": "cy", "home": {"city": "Oslo"}}]))
    handle(Customer(ident="2", name="bo", tier="pro", score=3, tags=["x"], address={"city": "Rome", "zip": "1"}, home=Address(city="Bern")))
