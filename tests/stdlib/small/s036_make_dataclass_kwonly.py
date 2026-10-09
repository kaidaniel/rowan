# make_dataclass, inherited field order, KW_ONLY; a generic helper rebuilding any dataclass
from dataclasses import KW_ONLY, asdict, dataclass, field, fields, make_dataclass

Item = make_dataclass("Item", ["sku", ("qty", int, field(default=0))])


@dataclass
class Base:
    id: int
    created: str = "now"


@dataclass
class Child(Base):
    name: str = ""
    _: KW_ONLY
    flag: bool = False


def clone_with(obj, **changes):
    d = asdict(obj)
    d.update(changes)
    _ = 0                                        #@row d: present={sku,qty,id,created,name,flag} closed
    return type(obj)(**d)


if __name__ == "__main__":
    i = Item("a")                                #@row i: present={sku,qty} definite={sku,qty} closed
    q = i.qty                                    #@value q: exactly={0}
    c = Child(1, "t", "n", flag=True)            #@row c: present={id,created,name,flag} definite={id,created,name,flag} closed
    order = tuple(f.name for f in fields(Child)) #@value order: exactly={("id","created","name","flag")}
    bad = False
    try:
        Child(1, "t", "n", True)
    except TypeError:
        bad = True
    #@value bad: exactly={True}
    i2 = clone_with(i, qty=5)                    #@row i2: present={sku,qty} definite={sku,qty} closed
    c2 = clone_with(c, name="m")
    nm = c2.name                                 #@value nm: exactly={m}
    try:
        i2.flag                                  #@ missing flag
    except AttributeError:
        pass
