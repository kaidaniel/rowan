"""Hypothesis: `__init_subclass__` runs once per subclass definition with the subclass as
`cls`, so a registry keyed by `cls.__name__` has exactly the subclasses.

Expectations were written from the program text before running the engine."""

REG = {}


class Plugin:
    def __init_subclass__(cls, **kw):
        super().__init_subclass__(**kw)
        REG[cls.__name__] = cls


class A(Plugin):
    pass


class B(Plugin):
    pass


_ = 0                                        #@row REG: present={A,B} definite={A,B} closed
REG["Plugin"]                                #@ missing Plugin
