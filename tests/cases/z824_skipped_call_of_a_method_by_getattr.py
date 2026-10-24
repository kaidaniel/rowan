"""A call whose function's budget refused it does not run: what it may change comes from its
function's text. A method of an argument called through `getattr(o, "m")(…)`, or read into a
name first (`f = o.m`, `f = getattr(o, "m")`) and called, may change what that method changes
on its receiver: here `v`, which the instance did not have before."""
#@limits budget=0


class B:
    def m(self, a):
        self.v = len(a)


def by_getattr(a, o):
    getattr(o, "m")(a)


def by_name(a, o):
    f = o.m
    f(a)


def by_getattr_name(a, o):
    f = getattr(o, "m")
    f(a)


by_getattr([], B())
o1 = B()
by_getattr([0], o1)
n = o1.v if hasattr(o1, "v") else -1
#@value n: includes={1}

by_name([], B())
o2 = B()
by_name([0], o2)
n = o2.v if hasattr(o2, "v") else -1
#@value n: includes={1}

by_getattr_name([], B())
o3 = B()
by_getattr_name([0], o3)
n = o3.v if hasattr(o3, "v") else -1
#@value n: includes={1}
