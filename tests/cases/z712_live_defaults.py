"""A function's `__defaults__` and `__kwdefaults__` are live: what the program stores in them,
or through them, is what its calls take (networkx's `argmap` fills its wrapper's keyword-only
default with the wrapper itself, then calls it without that argument). Before, calls took the
defaults of the `def` alone: `r` was None, `r2` 1, and the wrapper's call always raised.

Expectations were written from the program text before running the engine."""
import os


def f(*, w=None):
    return w


f.__kwdefaults__["w"] = 5
r = f()                                      #@value r: exactly={5}


def g(a, b=1):
    return b


g.__defaults__ = (2,)
r2 = g(0)                                    #@value r2: exactly={2}
d = g.__defaults__                           #@value d: exactly={(2,)}


def h(x, *, y=3):
    return y


kd = h.__kwdefaults__                        #@row kd: present={y} definite={y} closed
dd = h.__defaults__                          #@value dd: exactly={None}
r3 = h(1)                                    #@value r3: exactly={3}


def make():
    def inner(*args, wrapper=None):
        return wrapper.__name__

    inner.__kwdefaults__["wrapper"] = inner
    return inner


wrapped = make()
name = wrapped()                             #@value name: exactly={"inner"}

FLAG = os.environ.get("ROWAN_Z712") == "1"


def maybe():
    def k(v=1):
        return v

    if FLAG:
        k.__defaults__ = (7,)
    return k()


m = maybe()                                  #@value m: exactly={1, 7}

# (a `def` run again makes a new function: what the earlier one was given is not its)
outs = []
for i in range(2):
    def again(v=1):
        return v

    if i == 0:
        again.__defaults__ = (7,)
    outs.append(again())
first, second = outs                         #@value first: exactly={7}
#@value second: exactly={1}
