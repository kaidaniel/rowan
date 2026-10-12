"""`self.x = v` on an instance whose class defines `__setattr__` calls it. In a method's summary
`self` is a placeholder of the class: the hook was looked up on instances only, found nothing,
and the store was dropped, so what `__init__` set through `__setattr__` read as definitely
missing (`_pydecimal.Context`, whose `__setattr__` checks each value and stores it with
`object.__setattr__`). With the tabulation budget at 0 every call uses its summary."""
# (summaries are gone since: every call runs at its call site)


class Ctx:
    def __init__(self, prec=None):
        self.prec = prec if prec is not None else 28
        self.flags = {}

    def _set_integer_check(self, name, value, vmin):
        if not isinstance(value, int):
            raise TypeError(name)
        if value < vmin:
            raise ValueError(name)
        return object.__setattr__(self, name, value)

    def __setattr__(self, name, value):
        if name == "prec":
            return self._set_integer_check(name, value, 1)
        elif name == "flags":
            return object.__setattr__(self, name, value)
        raise AttributeError(name)


class Plain:
    def __init__(self):
        self.size = 3

    def __setattr__(self, name, value):
        self.__dict__[name] = value


c = Ctx()
p = c.prec  #@ sound prec
f = c.flags  #@ sound flags
s = Plain().size  #@ sound size
print(p, f, s)
