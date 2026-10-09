# dataclasses.replace / fields / field(default_factory=..., init=False) and attributes added later
from dataclasses import dataclass, field, fields, replace


@dataclass
class Order:
    id: int
    items: list = field(default_factory=list)
    meta: dict = field(default_factory=dict)
    status: str = "new"
    total: float = field(init=False, default_factory=float)


def ship(o):
    s = replace(o, status="shipped")             #@row s: present={id,items,meta,status,total} definite={id,items,meta,status,total} closed
    st = s.status                                #@value st: exactly={shipped}
    return s


def annotate(o, **extra):
    o.meta.update(extra)
    return o.meta


if __name__ == "__main__":
    names = tuple(f.name for f in fields(Order)) #@value names: exactly={("id","items","meta","status","total")}
    o = Order(1)
    p = Order(2, ["x"])
    ship(o)
    ship(p)
    fresh = o.items is not p.items               #@value fresh: exactly={True}
    m1 = annotate(o, source="web")               #@row m1: present={source} definite={source} closed
    m2 = annotate(p)                             #@row m2: present={} closed
    p.note = "late"
    _ = 0                                        #@row p: present={id,items,meta,status,total,note} definite={id,items,meta,status,total,note} closed
    try:
        o.note                                   #@ missing note
    except AttributeError:
        pass
    q = replace(p, id=3)
    try:
        q.note                                   #@ missing note
    except AttributeError:
        pass
