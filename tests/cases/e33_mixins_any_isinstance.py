"""Entry-point analysis of library code: mixins have no attributes of their own, `Any` fields
are not checkable, isinstance guards on attribute chains narrow the attribute."""
from dataclasses import dataclass
from typing import Any, Optional


class Mixin:
    def describe(self):
        return self.name                          #@ input name


class Thing(Mixin):
    def __init__(self, name):
        self.name = name


@dataclass
class Obj:
    kind: str
    payload: Any = None


@dataclass
class Info:
    tree: int


class Lit:
    def __init__(self, key):
        self.key = key


class KeysOf:
    def __init__(self, objs):
        self.objs = objs


class Holder:
    def __init__(self, kv, obj: Obj):
        self.kv = kv
        self.obj = obj

    def objs(self):
        if isinstance(self.kv, KeysOf):
            return self.kv.objs                   #@ ok objs ; none key
        return None

    def tree(self):
        mi = self.obj.payload
        return mi.tree                            #@ none tree


h = Holder(Lit("a"), Obj("x", Info(1)))
h.objs()
h2 = Holder(KeysOf([1]), Obj("y", "not info"))
h2.objs()
h2.tree()
