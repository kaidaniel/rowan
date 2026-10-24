"""A failing `assert` raises AssertionError (with its message evaluated first): a handler of it
runs, and code after the try goes on."""
import sys

log = []


def check(x):
    try:
        assert x is not None, log.append("message")
        state = "ok"
    except AssertionError:
        state = "failed"
    return state


r = check(None)
#@value r: exactly={"failed"}
n = len(log)
#@value n: exactly={1}
s = check(len(sys.argv))
#@value s: includes={"ok"}
#@todo value s: exactly={"ok"}  (an int(?) is not None: `is not None` of a typed unknown is not decided)


def strict(x):
    assert x > 0
    return x


try:
    strict(-1)
    t = "returned"
except AssertionError:
    t = "raised"
#@value t: exactly={"raised"}
