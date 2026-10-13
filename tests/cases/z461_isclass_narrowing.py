"""Hypothesis: `inspect.isclass(x)` narrows x: in its true branch x is a class (an instance
or a literal it held is gone), in its false branch not one. skbase reads `obj.__name__`
behind `isclass(obj)` for classes and instances alike: rowan said ✗ for the instances.

Expectations were written from the program text before running the engine."""

import sys
from inspect import isclass
import inspect


class Model:
    pass


def name_of(obj):
    if isclass(obj):
        return obj.__name__                  #@ sound __name__
    return type(obj).__name__


def kind(obj):
    if not inspect.isclass(obj):
        return "instance"
    return "class"


pick = Model if len(sys.argv) > 5 else Model()
n = name_of(pick)                            #@value n: exactly={Model}
k = kind(pick)                               #@value k: exactly={instance, class}
c = kind(Model)                              #@value c: exactly={class}
