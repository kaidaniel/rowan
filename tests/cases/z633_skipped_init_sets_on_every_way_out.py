"""A skipped `__init__` (here every call is: `#@limits budget=0`) sets on its receiver the
attributes its code stores on every way it completes: before an early `return` as at its end
(a branch that raises does not complete), and through `super().__init__(…)` as the base's
`__init__` does. Those reads always succeed; rowan said they may fail (`T(text)` had no
attributes on the path through `super().__init__`, pip's `_TrimmedRelease`).

Expectations were written from the program text before running the engine."""
#@limits budget=0
import os


class V:
    def __init__(self, text):
        if text.isdigit():
            try:
                self.rel = int(text)
            except ValueError:
                raise
            self.epoch = 0
            self.cache = None
            return
        if not text:
            raise ValueError(text)
        self.epoch = 1
        self.rel = len(text)
        self.cache = None
        if text == "x":
            self.extra = 1


class T(V):
    def __init__(self, v):
        if isinstance(v, V):
            self.epoch = v.epoch
            self.rel = v.rel
            self.cache = v.cache
            return
        super().__init__(v)

    def key(self):
        return (self.epoch, self.rel, self.cache)  #@ ok epoch ; ok rel ; ok cache


t = T(os.environ.get("Z633_V", "12"))
print(t.epoch, t.rel)  #@ ok epoch ; ok rel
try:
    print(t.extra)  #@ possibly extra
except AttributeError:
    pass
u = T(t)
print(u.cache)  #@ ok cache
print(u.key())
