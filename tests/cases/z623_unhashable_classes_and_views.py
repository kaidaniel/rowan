"""A class that sets `__hash__ = None`, or whose body defines `__eq__` without `__hash__`
(CPython then sets `__hash__` to None), has unhashable instances; a subclass inherits that
unless it sets `__hash__` again. A dict's keys and items views are set-like and unhashable;
its values view hashes (by identity). Rowan took all of these to hash.

Expectations were written from the program text before running the engine."""


class NoHash:
    __hash__ = None


class EqOnly:
    def __eq__(self, other):
        return True


class Rehashed(EqOnly):
    def __hash__(self):
        return 1


class Plain:
    pass


def tag(x):
    try:
        {x: 1}
        return "hashed"
    except TypeError:
        return "unhashable"


def member(x):
    try:
        s = set()
        s.add(x)
        return "hashed"
    except TypeError:
        return "unhashable"


d = {"a": 1}
t1 = tag(NoHash())                       #@value t1: exactly={"unhashable"}
t2 = tag(EqOnly())                       #@value t2: exactly={"unhashable"}
t3 = tag(Rehashed())                     #@value t3: exactly={"hashed"}
t4 = tag(Plain())                        #@value t4: exactly={"hashed"}
t5 = tag(d.keys())                       #@value t5: exactly={"unhashable"}
t6 = tag(d.items())                      #@value t6: exactly={"unhashable"}
t7 = tag(d.values())                     #@value t7: exactly={"hashed"}
m1 = member(NoHash())                    #@value m1: includes={"unhashable"}
m2 = member(d.keys())                    #@value m2: includes={"unhashable"}
m3 = member(Plain())                     #@value m3: includes={"hashed"}
