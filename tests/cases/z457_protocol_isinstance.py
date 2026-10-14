"""Hypothesis: `isinstance(x, P)` with a runtime-checkable `Protocol` class P is structural:
an instance whose class defines P's members is an instance of P without deriving from it.
rowan decided it by the class hierarchy only, so rich's `isinstance(renderable,
ConsoleRenderable)` was False for a Table and printing it went another way.

Expectations were written from the program text before running the engine."""

from typing import Protocol, runtime_checkable


@runtime_checkable
class Renderable(Protocol):
    def render(self) -> str: ...


@runtime_checkable
class Measured(Renderable, Protocol):
    size: int


class Table:
    def render(self):
        return "table"


class Plain:
    pass


class Box(Renderable):
    def render(self):
        return "box"


def show(x):
    if isinstance(x, Renderable):
        return x.render()
    return "plain"


a = show(Table())                            #@value a: exactly={table}
b = show(Plain())                            #@value b: includes={plain}
#@todo value b: exactly={plain}
c = show(Box())                              #@value c: exactly={box}
d = isinstance(Table(), Measured)            #@value d: includes={False}
#@todo value d: exactly={False}
