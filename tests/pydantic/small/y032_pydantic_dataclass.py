# pydantic.dataclasses.dataclass: validated dataclass instances (attributes = fields), dataclasses.asdict, extra kwargs ignored, missing ones fail
import dataclasses

from pydantic import TypeAdapter, ValidationError
from pydantic.dataclasses import dataclass


@dataclass
class Point:
    x: int
    y: int = 0
    tags: list[str] = dataclasses.field(default_factory=list)


@dataclass
class Segment:
    a: Point
    b: Point
    name: str = "seg"


def point(d):
    p = Point(**d)                               #@ possibly x
    #@row p: present={x,y,tags} definite={x,y,tags} absent={z} closed
    return p


def segment(d):
    s = TypeAdapter(Segment).validate_python(d)
    #@row s: present={a,b,name} definite={a,b,name} closed
    sa = s.a
    #@row sa: present={x,y,tags} definite={x,y,tags} closed
    d2 = dataclasses.asdict(s)
    #@row d2: present={a,b,name} definite={a,b,name} closed
    db = d2["b"]                                 #@ ok b
    #@row db: present={x,y,tags} definite={x,y,tags} closed
    return d2


p = Point(x="3")
px = p.x
#@value px: exactly={3}
pd = dataclasses.asdict(p)
#@row pd: present={x,y,tags} definite={x,y,tags} closed

if __name__ == "__main__":
    point({"x": 1})
    point({"x": 2, "y": 3, "z": 4})
    try:
        point({"y": 5})
    except ValidationError:
        pass
    segment({"a": {"x": 1}, "b": {"x": 2, "y": 2}})
    segment({"a": {"x": 0}, "b": {"x": 1}, "name": "n"})

#@schema param point.d: always={x} only={x}
#@schema param segment.d: always={a,b} only={a,b}
#@schema param segment.d['a']: always={x} only={x}
