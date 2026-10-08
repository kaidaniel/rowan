#@engine rust
"""Inheritance, super(), properties, class/static methods, dunder protocols, class attributes,
attributes set outside __init__, isinstance narrowing."""
from dataclasses import dataclass, field


class Base:
    registry = {"base": True}

    def __init__(self, cfg):
        self.cfg = cfg
        self.name = cfg["name"]          #@ ok name

    @property
    def label(self):
        return self.cfg.get("label", self.name)

    @classmethod
    def make(cls, name):
        return cls({"name": name})

    @staticmethod
    def default_cfg():
        return {"name": "default"}

    def describe(self):
        return {"name": self.name, "kind": self.kind()}

    def kind(self):
        return "base"


class Child(Base):
    registry = {"child": True}

    def __init__(self, cfg):
        super().__init__(cfg)
        self.extra = cfg.get("extra")

    def kind(self):
        return "child"

    def finish(self):
        self.done = True


b = Base.make("b")
b.name                                   #@ ok name
b.label                                  #@ none label
b.extra                                  #@ missing extra
c = Child({"name": "c"})
c.extra                                  #@ ok extra
c.done                                   #@ missing done
c.finish()
c.done                                   #@ ok done
c.describe()["kind"]                     #@ ok kind
c.describe()["size"]                     #@ missing size
Base.registry["base"]                    #@ ok base
Child.registry["base"]                   #@ missing base
Child.registry["child"]                  #@ ok child
Base.default_cfg()["name"]               #@ ok name
Base.default_cfg()["label"]              #@ missing label


class Mapping:
    def __init__(self):
        self._d = {"k": 1}

    def __getitem__(self, k):
        return self._d[k]

    def __setitem__(self, k, v):
        self._d[k] = v

    def __delitem__(self, k):
        del self._d[k]

    def __contains__(self, k):
        return k in self._d

    def __call__(self, k):
        return self._d[k]


mp = Mapping()
mp["k"]                                  #@ ok k
mp["j"]                                  #@ missing j
mp["j"] = 2
mp["j"]                                  #@ ok j
del mp["k"]
mp["k"]                                  #@ missing k
# __contains__ narrows self._d, which is closed without "z": the body is dead code
if "z" in mp:
    mp["z"]                              #@ none z
mp("j")                                  #@ ok j
mp("q")                                  #@ missing q


@dataclass
class Point:
    x: int
    y: int = 0
    tags: list = field(default_factory=list)


pt = Point(1)
pt.x                                     #@ ok x
pt.y                                     #@ ok y
pt.tags                                  #@ ok tags
pt.z                                     #@ missing z


@dataclass
class Point3(Point):
    z: int = 0


p3 = Point3(1, 2, [], 3)
p3.z                                     #@ ok z
p3.x                                     #@ ok x
p3.w                                     #@ missing w


def use(obj):
    if isinstance(obj, Child):
        return obj.extra                 #@ ok extra
    if isinstance(obj, Base):
        return obj.name                  #@ ok name
    return obj["key"]


use(c)
use(b)
use({"key": 1})                          #@ ok key
use({"other": 1})                        #@ missing key


class Counter:
    total = 0

    def __init__(self):
        Counter.total += 1
        self.id = Counter.total


cc = Counter()
cc.id                                    #@ ok id
# class attributes are not records: no verdict
Counter.total                            #@ none total
