"""A call past its budget gives back what its function returns for any call (its result
summary), and a run for any call takes each parameter as its annotation says. A call passing an
instance of a class the annotation does not name is no such call: its result is not known.
Before, `b` and `c` were "foo" alone, the annotated class's answer, where the argument is (or
may be) a Bar. A call that fits the annotation still gets the summary (`d`).

Expectations were written from the program text before running the engine."""
#@limits budget=0
import sys


class Foo:
    def first(self):
        return "foo"


class Bar:
    def first(self):
        return "bar"


def use(x: Foo):
    return x.first()


def use_opt(x: Foo | None):
    return "none" if x is None else x.first()


a = use(Foo())
#@value a: exactly={"foo"}
b = use(Bar())
#@value b: includes={"bar"}
c = use(Foo() if len(sys.argv) > 2 else Bar())
#@value c: includes={"foo", "bar"}
d = use(Foo())
#@value d: exactly={"foo"}
e = use_opt(None)
#@value e: exactly={"none"}
f = use_opt(Bar())
#@value f: includes={"bar"}
g = use_opt(None)
#@value g: includes={"none"}
#@todo value g: exactly={"none"}  (the summary is the run for any call's: x is None or a Foo there)
