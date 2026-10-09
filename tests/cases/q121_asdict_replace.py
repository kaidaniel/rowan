"""Hypothesis: `dataclasses.asdict` gives the field keys (nested dataclasses too) and
`dataclasses.replace` gives a new instance with the same fields.

Expectations were written from the program text before running the engine."""
from dataclasses import asdict, dataclass, field, replace


@dataclass
class Addr:
    city: str
    zip: str = ""


@dataclass
class User:
    name: str
    addr: Addr
    tags: list = field(default_factory=list)


u = User("ann", Addr("Berlin"))
d = asdict(u)                             #@row d: present={name,addr,tags} definite={name,addr,tags} closed
u2 = replace(u, name="bob")               #@row u2: present={name,addr,tags} definite={name,addr,tags} closed
d["email"]                                #@ missing email
u2.email                                  #@ missing email
