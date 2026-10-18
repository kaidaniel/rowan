# A raise in code a try runs (a callee, a with body of the `@contextmanager` generator the try
# is in) reaches the handler with the try's own names, as bound as they were there: a name the
# raising code leaves possibly unbound (`param` in `inner`, after a loop) is another frame's.
import sys
from contextlib import contextmanager


@contextmanager
def augment(param=None):
    try:
        yield
    except KeyError:
        if param is not None:
            #@ none param
            print("with", param)
        raise


def inner(xs):
    for param in xs:
        print(param)
    raise KeyError("k")


def outer(xs):
    with augment(param=1):
        inner(xs)


def direct(xs):
    name = "n"
    try:
        inner(xs)
    except KeyError:
        print(name)
        #@ none name


def inner2(xs):
    for name in xs:
        print(name)
    raise KeyError("k")


def direct2(xs):
    name = "n"
    try:
        inner2(xs)
    except KeyError:
        print(name)
        #@ none name


try:
    outer(sys.argv[1:])
except KeyError:
    pass
direct(sys.argv[1:])
direct2(sys.argv[1:])
