"""Hypothesis: subscripting a generic class (`Tree["Token"]`, lark) gives an alias that
makes instances of the class, and every module has `__name__` / `__file__` / `__doc__`
(hypothesis reads `package.__file__`). rowan said both were never set.

Expectations were written from the program text before running the engine."""

import json
from typing import Generic, TypeVar

T = TypeVar("T")


class Tree(Generic[T]):
    def __init__(self, data):
        self.data = data


alias = Tree["Token"]
t = alias("x")
d = t.data                                   #@value d: exactly={x}
name = json.__name__
mod = __name__                               #@value mod: exactly={__main__}
here = Tree.__module__
