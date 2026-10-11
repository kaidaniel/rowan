"""Every instance of a user class has `object`'s attributes (`__class__`, `__repr__`, `__eq__`,
`__hash__`, …) and its class's `__module__` / `__doc__`, and `__dict__` / `__weakref__` when the
class has no `__slots__`, whatever its own attributes are. They were read as definitely
missing: at module level, and in a method's summary replayed on the instance (stdlib12k:
`self.__class__` in pydoc's `Helper`)."""


class H:
    pass


class Helper:
    def name(self):
        return self.__class__.__name__, self.__module__  #@ sound __class__ ; sound __module__


h = H()
print(h.__module__, h.__doc__, h.__repr__(), h.__weakref__)  #@ sound __module__ ; sound __doc__ ; sound __repr__ ; sound __weakref__
print(h.__eq__(h), h.__hash__())  #@ sound __eq__ ; sound __hash__
print(Helper().name())
