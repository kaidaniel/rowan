"""A class with `__getattr__` whose `__init__` sets its attribute through
`object.__setattr__(self, "v", v)`: where that `__init__` does not run (past its budget), `v`
may be there after it — a read finds it or, where it is not, `__getattr__` answers. (Before,
the read went to `__getattr__` alone: `d.v` was 0, where the program reads 2.)

Expectations were written from the program text before running the engine."""
#@limits budget=0


class G:
    def __init__(self, v):
        object.__setattr__(self, "v", v)

    def __getattr__(self, name):
        return 0


c = G(1)
d = G(2)
got = d.v
#@value got: includes={2}
