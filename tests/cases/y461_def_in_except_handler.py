"""A function (or class, lambda) defined inside an `except` handler has its own scope: the
module's names it reads are globals. The scope pass walked a copy of each handler, so such a
function's scope was never found and it took the module's scope: every module name it read was
a local, unbound in its frame, and reported as definitely missing (rich's
`get_windows_console_features`, defined in the handler of a failed Windows import, returning
`WindowsConsoleFeatures()`)."""
import sys

LIMIT = 3


class Features:
    vt = False


try:
    if sys.platform != "win32":
        raise ImportError("Not windows")
    vt = True
except ImportError:
    def get_features():
        f = Features()  #@ sound Features
        f.limit = LIMIT  #@ sound LIMIT
        return f

    scale = lambda n: n * LIMIT  # noqa: E731
else:
    def get_features():
        return Features()

    def scale(n):
        return n

feat = get_features()
s = scale(2)  #@value s: includes={6}
print(feat, s)
