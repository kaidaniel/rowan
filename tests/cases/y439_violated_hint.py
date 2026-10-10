"""Type hints are not trusted where the argument's class is known: a call that passes an
object of another class gets that class's behaviour (SUGGESTED_CHANGES 2: soundness first).
The hint still types arguments whose class is unknown."""
import sys
from dataclasses import dataclass


class Foo:
    def first(self):
        return "foo"


class Bar:
    def first(self):
        return "bar"


def use(x: Foo):
    return x.first()


a = use(Bar())
#@value a: exactly={"bar"}
b = use(Foo())
#@value b: exactly={"foo"}
c = use(Foo() if len(sys.argv) > 2 else Bar())
#@value c: exactly={"foo", "bar"}


class P:
    def __init__(self):
        self.name = "p"


class Q:
    def __init__(self):
        self.title = "q"


def label(o: P):
    return o.name
#@ possibly name


l1 = label(P())
#@value l1: exactly={"p"}
try:
    l2 = label(Q())
except AttributeError:
    l2 = None
#@value l2: exactly={None}


@dataclass
class Box:
    item: Foo


def peek(bx: Box):
    return bx.item.first()


p = peek(Box(Bar()))
#@value p: exactly={"bar"}
