"""A class taken from a mapping of classes (a registry with a default, as email's
`HeaderRegistry.__getitem__` does) has `__name__`: every class has the attributes of `type`
(`__name__`, `__qualname__`, `__module__`, `__mro__`, `mro`, …) whatever its body defines. The
function's summary records the access on the value `get` returned; replayed on the classes a
call passes, it was classified against their attribute rows (which list the body's names
only) and reported `__name__` definitely missing."""
import sys


class A:
    max_count = 1


class B(A):
    pass


class C:
    pass


registry = {"a": A, "b": B}


def name_of(key):
    cls = registry.get(key.lower(), C)
    return "_" + cls.__name__  #@ sound __name__


def module_of(key):
    cls = registry.get(key, C)
    return cls.__module__, cls.__qualname__, cls.mro()  #@ sound __module__ ; sound __qualname__ ; sound mro


n1 = name_of(sys.argv[0])  #@value n1: includes={"_A","_B","_C"}
n2 = name_of("a")  #@value n2: exactly={"_A"}
m = module_of(sys.argv[0])
print(n1, n2, m)
