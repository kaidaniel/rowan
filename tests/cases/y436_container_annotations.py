"""Hypothesis: a container annotation (list[Foo], dict[str, Foo], set[Foo], tuple[Foo, ...])
does not make the parameter a Foo: its methods are the container's; Optional[Foo] and
Foo | None still type the value as a Foo where it is one."""
from typing import Optional


class Foo:
    def count(self, x):
        return "foo-count"

    def get(self, k, d=None):
        return "foo-get"

    def first(self):
        return "foo-first"


def f(xs: list[Foo]):
    return xs.count(1)


def g(m: dict[str, Foo]):
    return m.get("k", "dflt")


def h(o: Optional[Foo]):
    return o.first() if o is not None else "none"


def k(o: Foo | None):
    return o.first() if o else "none"


a = f([Foo(), Foo()])                             #@value a: exactly={0}
b = g({})                                         #@value b: exactly={"dflt"}
c = h(Foo())                                      #@value c: exactly={"foo-first"}
d = h(None)                                       #@value d: exactly={"none"}
e = k(Foo())                                      #@value e: exactly={"foo-first"}
