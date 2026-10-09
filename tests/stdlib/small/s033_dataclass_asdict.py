# dataclasses.asdict / astuple of nested dataclasses and of a subclass: the fields become keys
from dataclasses import asdict, astuple, dataclass, field


@dataclass
class Address:
    city: str
    zip: str = ""


@dataclass
class Person:
    name: str
    address: Address
    tags: list = field(default_factory=list)


@dataclass
class Employee(Person):
    dept: str = "eng"


def export(p):
    d = asdict(p)                                #@row d: present={name,address,tags,dept} definite={name,address,tags} closed
    a = d["address"]                             #@row a: present={city,zip} definite={city,zip} closed
    d["address"]["city"]                         #@ ok city
    t = astuple(p)
    n = len(t)                                   #@value n: exactly={3,4}
    return d


if __name__ == "__main__":
    e1 = export(Person("ann", Address("Paris")))
    e2 = export(Employee("bob", Address("Oslo", "0150"), ["x"], "ops"))
    dp = e2["dept"]                              #@value dp: exactly={ops}
    try:
        e1["dept"]                               #@ missing dept
    except KeyError:
        pass
    t = astuple(Address("Rome"))                 #@value t: exactly={("Rome","")}
    z = asdict(Address("Lima", "15"))            #@row z: present={city,zip} definite={city,zip} closed
