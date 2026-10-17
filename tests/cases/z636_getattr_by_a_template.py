"""`getattr(obj, name)` where `name` is made by a template (`"_on_%s" % mode`, `f"_on_{mode}"`,
`"_on_{}".format(mode)`), written in the call or assigned to a local only ever assigned
templates, is one of the attributes whose names fit the template (argparse's
`_handle_conflict_%s`). Rowan picked every attribute when the template's field was not known:
here `clear` too, so the dict looked emptied. A local also assigned something else may be any
attribute.

Expectations were written from the program text before running the engine."""
import os


class Box:
    def __init__(self, mode):
        self.mode = mode
        self.items = {"k": 1}

    def _on_keep(self):
        return "kept"

    def _on_drop(self):
        return "dropped"

    def clear(self):
        self.items.clear()
        return "cleared"

    def run_pct(self):
        name = "_on_%s" % self.mode
        return getattr(self, name)()

    def run_f(self):
        return getattr(self, f"_on_{self.mode}")()

    def run_fmt(self):
        return getattr(self, "_on_{}".format(self.mode))()

    def run_two(self, other):
        name = "_on_%s" % self.mode
        if other:
            name = other
        return getattr(self, name)()


b = Box(os.environ.get("Z636_MODE", "keep"))
r1 = b.run_pct()
print(r1)  #@value r1: exactly={"kept", "dropped"}
r2 = b.run_f()
print(r2)  #@value r2: exactly={"kept", "dropped"}
r3 = b.run_fmt()
print(r3)  #@value r3: exactly={"kept", "dropped"}
print(b.items["k"])  #@ ok k
c = Box(os.environ.get("Z636_MODE", "keep"))
r4 = c.run_two(os.environ.get("Z636_OTHER"))
print(r4)  #@value r4: includes={"kept", "dropped", "cleared"}
print(c.items.get("k"))
