# A handler binds only the raised exceptions of its classes: `except BadParameter as e` after a
# re-raise that may carry an Exit (`except Exception: raise` in a callee) never sees the Exit,
# so `e.ctx` (set by every BadParameter) cannot fail. python z675.py → n = 1: "exit".
import sys


class Exit(RuntimeError):
    pass


class BadParameter(Exception):
    def __init__(self, msg):
        super().__init__(msg)
        self.ctx = None


def go(n):
    if n == 1:
        raise Exit()
    if n == 2:
        raise BadParameter("bad")
    return n


def wrap(n):
    try:
        return go(n)
    except Exception:
        raise


def aug(n):
    try:
        return wrap(n)
    except BadParameter as e:
        return e.ctx
        #@ ok ctx
    except Exit:
        return "exit"


r = aug(len(sys.argv))
q = r
#@value q: includes={"exit"}
