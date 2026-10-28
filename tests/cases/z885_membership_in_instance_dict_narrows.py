"""`"x" in obj.__dict__` / `"x" not in obj.__dict__` say whether the instance itself holds the
attribute `x` (its own dict, not its class's): where the test holds, `obj.x` is the instance's
own value; where it fails, the instance has no `x` of its own, so `obj.x` is the class's (or
an AttributeError, or `__getattr__`). hypothesis's `ThreadLocal.__setattr__` tests
`"_ThreadLocal__initialized" not in self.__dict__ or not self.__initialized`: the read on the
right of the `or` runs only where the attribute is there. Before, the test narrowed nothing:
both sides kept "may be there", and a class's `__getattr__` ran where it cannot.

Expectations were written from the program text before running the engine."""
import sys


class Lazy:
    def __init__(self, ready):
        if ready:
            self.value = 10


def read_if_set(o):
    if "value" in o.__dict__:
        got = o.value                    #@ ok value
        return got
    return None


def read_if_unset(o):
    if "value" not in o.__dict__:
        return o.value                   #@ missing value
    return o.value                       #@ ok value


lazy = Lazy(len(sys.argv) > 5)
#@row lazy: present={value} definite={} closed
r = read_if_set(lazy)
#@value r: exactly={10, None}
try:
    u = read_if_unset(lazy)
except AttributeError:
    u = -1
got_u = u
#@value got_u: exactly={10, -1}


class WithDefault:
    mode = "class"

    def __init__(self, own):
        if own:
            self.mode = "own"


def mode_of(o):
    if "mode" not in o.__dict__:
        m = o.mode                       #@ ok mode
        #@value m: exactly={"class"}
    else:
        m = o.mode                       #@ ok mode
        #@value m: exactly={"own"}
    return m


picked = mode_of(WithDefault(len(sys.argv) > 5))
#@value picked: exactly={"class", "own"}


class Local:
    def __init__(self, **defaults):
        self.__ready = False
        self.__defaults = defaults
        self.__ready = True

    def __getattr__(self, name):
        if name not in self.__defaults:
            raise AttributeError(name)
        return self.__defaults[name]

    def __setattr__(self, name, value):
        if "_Local__ready" not in self.__dict__ or not self.__ready:  #@ ok _Local__ready
            super().__setattr__(name, value)
        else:
            raise AttributeError(name)


loc = Local(a=1)
#@row loc: present={_Local__ready, _Local__defaults} definite={_Local__ready, _Local__defaults} closed
v = loc.a
#@value v: exactly={1}
try:
    loc.b = 2
    blocked = False
except AttributeError:
    blocked = True
got_blocked = blocked
#@value got_blocked: exactly={True}
