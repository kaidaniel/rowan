"""A fake installed package for z713_cut_class_hooks.py: a base whose `__init_subclass__`
maps a subclass's annotated fields to class attributes by computed names (as SQLAlchemy's
declarative base does), after work past an entry's allowance under `#@limits budget=64`;
its `__init__` accepts only keyword arguments the class has."""

ROWS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]


def _cell(i, j):
    return i * j


def _expensive():
    out = 0
    for i in ROWS:
        for j in ROWS:
            out += _cell(i, j)
    return out


class Field:
    def __init__(self, name):
        self.name = name


def _map(cls):
    for name in list(cls.__annotations__):
        setattr(cls, name, Field(name))


class Base:
    def __init_subclass__(cls, **kw):
        super().__init_subclass__(**kw)
        _expensive()
        _map(cls)

    def __init__(self, **kwargs):
        for k in kwargs:
            if not hasattr(type(self), k):
                raise TypeError(k)
            setattr(self, k, kwargs[k])
