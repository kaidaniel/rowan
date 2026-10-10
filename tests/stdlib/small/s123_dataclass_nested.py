"""Hypothesis: dataclasses.asdict recurses into nested dataclasses (a nested dict keyed by the
inner field names) and copies, so mutating the dict leaves the instance alone; replace() is
shallow: the new instance shares the nested object and the default_factory list of the old one;
a field(default_factory=list) gives each new instance its own list. Exact attribute / key sets;
the environment record must have `name` and `city`."""
from dataclasses import asdict, dataclass, field, replace
import json
import os


@dataclass
class Addr:
    city: str
    zip: str = "00000"


@dataclass
class User:
    name: str
    addr: Addr
    tags: list = field(default_factory=list)
    meta: dict = field(default_factory=dict)


def make(rec):
    return User(rec["name"], Addr(rec["city"]), meta={"src": "rec"})


def rename(u, name):
    return replace(u, name=name)


def export(u):
    d = asdict(u)
    d["addr"]["zip"] = "x"
    d.pop("meta")
    return d


if __name__ == "__main__":
    u1 = make({"name": "ann", "city": "Oslo"})
    u2 = rename(u1, "bob")
    u2.tags.append("t")
    #@row u2: present={name,addr,tags,meta} definite={name,addr,tags,meta} closed
    t1 = len(u1.tags)                            #@value t1: exactly={1}
    same = u1.addr is u2.addr                    #@value same: exactly={True}
    n1 = u1.name                                 #@value n1: exactly={"ann"}
    u3 = User("c", Addr("Rome"))
    fresh = len(u3.tags)                         #@value fresh: exactly={0}
    e = export(u1)
    #@row e: present={name,addr,tags} definite={name,addr,tags} absent={meta} closed
    ea = e["addr"]
    #@row ea: present={city,zip} definite={city,zip} closed
    ez = ea["zip"]                               #@value ez: exactly={"x"}
    z = u1.addr.zip                              #@value z: exactly={"00000"}
    src = u2.meta["src"]                         #@value src: exactly={"rec"}
    rec = json.loads(os.environ.get("S123_U", '{"name": "e", "city": "Bergen", "x": 1}'))
    #@schema json.loads@L57: always={name,city} only={name,city}
    e4 = export(make(rec))
    #@row e4: present={name,addr,tags} definite={name,addr,tags} absent={meta} closed
