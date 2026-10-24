"""A method of an object rowan does not know is given its receiver when it is called: it may
change what the object holds, also what the program stored on it just before. So may the
method read into a name and called later, a list's reordering or emptying method on a
parameter rowan knows nothing of, and a library base class's method on the program's instance
(`logging.Handler.setLevel` sets `level`; a builtin base's, `list.append`, keeps attributes). For any call (`#@general`) the parameters are anything, so
the values read back after the call are not known; the program's own calls run their code."""
import logging
import threading


class B:
    v = 0

    def m(self):
        self.v = 1


def through_method(o):
    o.v = 2
    o.m()
    n = o.v
    #@value n: exactly={1}
    #@general n: unknown
    return n


def through_name(o):
    o.v = 2
    f = o.m
    f()
    n = o.v
    #@value n: exactly={1}
    #@general n: unknown
    return n


def reordered(a):
    a[0] = 5
    a.sort()
    n = a[0]
    #@value n: exactly={1}
    #@general n: unknown
    return n


def emptied(a):
    a.v = 2
    a.clear()
    n = a.v
    #@general n: unknown
    return n


through_method(B())
through_name(B())
reordered([9, 1])

ev = threading.Event()
ev._flag = 2
ev.set()
n = ev._flag
#@value n: unknown


class H(logging.Handler):
    def emit(self, record):
        pass


h = H()
h.level = 2
h.setLevel(10)
n = h.level
#@value n: unknown
