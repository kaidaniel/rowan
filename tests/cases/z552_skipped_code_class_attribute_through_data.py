"""A call past its budget that assigns an attribute of a class it reaches through data
(`REGISTRY[k].mode = ...`) or a call's result (`kind().mode = ...`) (the second call of each is past its budget) may rebind that class
attribute: it may hold anything after. rowan counted only names holding classes, `type(x)`
and parameters as the classes a skipped store may reach: the attribute kept its value.

Expectations were written from the program text before running the engine."""
#@limits budget=1


class A:
    mode = "x"


class B:
    mode = "x"


class C:
    mode = "x"


REGISTRY = {"a": A}


def kind():
    return B


def configure(k, m):
    a = 1
    REGISTRY[k].mode = m


def configure_b(m):
    a = 1
    kind().mode = m


def configure_all(m):
    a = 1
    for c in [C]:
        c.mode = m


configure("a", "x")
configure("a", "y")
configure_b("x")
configure_b("y")
configure_all("x")
configure_all("y")
ma = A.mode                              #@value ma: includes={"y"}
mb = B.mode                              #@value mb: includes={"y"}
mc = C.mode                              #@value mc: includes={"y"}
