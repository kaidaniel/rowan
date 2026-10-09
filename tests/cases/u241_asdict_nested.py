"""Hypothesis: `dataclasses.asdict` converts nested dataclasses into dicts too.

Expectations were written from the program text before running the engine."""

from dataclasses import asdict, dataclass


@dataclass
class Addr:
    city: str


@dataclass
class User:
    name: str
    addr: Addr


u = User("n", Addr("c"))
d = asdict(u)                                #@row d: present={name,addr} definite={name,addr} closed
a = d["addr"]                                #@row a: present={city} definite={city} closed
a["city"]                                    #@ ok city
