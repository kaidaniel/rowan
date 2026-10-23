"""A finally runs on every way out of its try: also when a handler raises (re-raising, or
another exception), when the `else` raises (its raise passes the handlers by), and when a
raise no handler catches meets a finally that returns (which swallows it)."""
import os

X = 0


def handler_reraises():
    global X
    X = 1
    try:
        raise KeyError("k")
    except LookupError:
        raise
    finally:
        X = X * 10 + 6


def handler_raises_other(c):
    global X
    X = 2
    try:
        if c:
            raise KeyError("k")
    except KeyError:
        raise ValueError("v")
    finally:
        X = X * 10 + 6


def else_raises():
    global X
    X = 3
    try:
        X = X * 10 + 1
    except KeyError:
        X = 0
    else:
        raise ValueError("v")
    finally:
        X = X * 10 + 6


def swallowed():
    global X
    X = 4
    try:
        raise ValueError("v")
    except KeyError:
        X = 0
    finally:
        X = X * 10 + 6
        return "done"


try:
    handler_reraises()
except KeyError:
    pass
a = X
#@value a: exactly={16}
try:
    handler_raises_other(os.environ.get("Z798", "yes") == "yes")
except ValueError:
    pass
b = X
#@value b: exactly={26}
try:
    else_raises()
except ValueError:
    pass
c = X
#@value c: exactly={316}
d = swallowed()
#@value d: exactly={"done"}
e = X
#@value e: exactly={46}
