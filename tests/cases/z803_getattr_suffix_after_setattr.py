"""getattr with a name built from a template (`self.project + "_mark"`, `f"{p}_mark"`) on a value
not known (a method of any plugin, for any call): it may hold any attribute, the mark stored by
a decorator under a name built at run time among them, so the read is not only the default.
An instance given an attribute under a built name (`setattr(b, prefix + "_x", 5)`): read back."""
import sys


class Marker:
    def __init__(self, project):
        self.project = project

    def __call__(self, fn):
        setattr(fn, self.project + "_mark", {"first": True})
        return fn


class Manager:
    def __init__(self, project):
        self.project = project

    def options(self, plugin, name):
        method = getattr(plugin, name)
        opts = getattr(method, self.project + "_mark", None)
        #@general opts: untyped
        found = getattr(method, f"{self.project}_mark", None)
        #@general found: untyped
        return opts, found


mark = Marker("demo")


class Plugin:
    @mark
    def hook(self):
        return 1


r = Manager("demo").options(Plugin(), "hook")


class Box:
    pass


prefix = sys.argv[0]
b = Box()
setattr(b, prefix + "_x", 5)
got = getattr(b, prefix + "_x", None)
#@value got: includes={5}
