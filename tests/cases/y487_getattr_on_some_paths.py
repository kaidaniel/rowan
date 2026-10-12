"""An attribute set on some paths of a class with `__getattr__`: the hook runs on the other
paths only; when it raises there, the paths that have the attribute go on."""
import sys


class Cfg:
    def __init__(self, flag):
        if flag:
            self.mode = "fast"

    def __getattr__(self, name):
        raise AttributeError(name)


c = Cfg(len(sys.argv) > 1)
m = c.mode                                   #@value m: exactly="fast"
after = 1                                    #@value after: exactly=1
