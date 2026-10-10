"""Hypothesis: `super().__init__` follows the MRO of the instance's class, not of the class the
method is written in: Leaf -> Mid -> TimestampMixin -> Base, so every level's attributes are set
once and the mixin's append happens after Base made the list; a class-level dict mutated in
__init__ is shared by all instances (keys from literal names exact, from input unknown); a
property setter on a __slots__ class stores through the slot (no other attribute can be set);
__getattr__ answers only for attributes the instance lacks."""
import json
import os


class Base:
    registry = {}

    def __init__(self, name):
        self.name = name
        self.tags = ["base"]


class TimestampMixin:
    def __init__(self, *a, **kw):
        super().__init__(*a, **kw)
        self.created = 0
        self.tags.append("ts")


class Mid(TimestampMixin, Base):
    def __init__(self, name, level=1):
        super().__init__(name)
        self.level = level
        Base.registry[name] = type(self).__name__


class Leaf(Mid):
    def __init__(self, name, extra=None):
        super().__init__(name, level=2)
        if extra:
            self.extra = extra


class Point:
    __slots__ = ("x", "_y")

    def __init__(self, x, y):
        self.x = x
        self.y = y

    @property
    def y(self):
        return self._y

    @y.setter
    def y(self, v):
        self._y = max(0, v)


class Config:
    def __init__(self, data):
        self.data = data

    def __getattr__(self, name):
        if name.startswith("_"):
            raise AttributeError(name)
        return self.data.get(name, "default")


def build(spec):
    obj = Leaf(spec["name"], spec.get("extra"))
    #@row obj: present={name,tags,created,level,extra} definite={name,tags,created,level} closed
    return obj


def extra_of(obj):
    return obj.extra                             #@ possibly extra


def try_set(obj, attr, v):
    try:
        setattr(obj, attr, v)
        return True
    except AttributeError:
        return False


if __name__ == "__main__":
    l1 = build({"name": "x"})
    #@row l1: present={name,tags,created,level} definite={name,tags,created,level} absent={extra} closed
    tg = tuple(l1.tags)                          #@value tg: exactly={("base","ts")}
    lv = l1.level                                #@value lv: exactly={2}
    l2 = build({"name": "y", "extra": 3})
    m = Mid("m")
    #@row m: present={name,tags,created,level} definite={name,tags,created,level} closed
    reg = dict(Base.registry)
    #@row reg: present={x,y,m} definite={x,y,m} closed
    rm = reg["m"] + reg["y"]                     #@value rm: exactly={"MidLeaf"}
    for o in (l1, l2):
        try:
            extra_of(o)
        except AttributeError:
            pass
    p = Point(1, -5)
    py = p.y                                     #@value py: exactly={0}
    p.y = 7
    py2 = p._y                                   #@value py2: exactly={7}
    s1 = try_set(p, "x", 9)                      #@value s1: exactly={True}
    s2 = try_set(p, "z", 1)                      #@value s2: exactly={False}
    c = Config({"mode": "fast"})
    #@row c: present={data} definite={data} closed
    mode = c.mode                                #@value mode: exactly={"fast"}
    lev = c.level                                #@ ok level
    _ = 0                                        #@value lev: exactly={"default"}
    build(json.loads(os.environ.get("SPEC", '{"name": "z", "extra": 0}')))
    #@schema json.loads@L111: always={name} only={name,extra}
    reg2 = dict(Base.registry)
    #@row reg2: present={x,y,m} definite={x,y,m} open
