# A skipped call's closure is walked in rounds: f reads `touch`, touch changes `items` (an
# append) and calls `reset`, which rebinds the class attribute `items` (a new list). The class
# attribute's flags were applied once, when `items` was first met (changed in place), so its
# rebinding by `reset`, met a round later, was lost: r was claimed to be the list stored at
# `C.items = [1]` (with any items), while Python gives the new list [9]. A walk now applies a
# name's flags again when they grow.
#@limits budget=1


class C:
    items = [1]

    def touch(self):
        self.items.append(2)
        self.reset()

    @classmethod
    def reset(cls):
        cls.items = [9]


def f(o):
    o.touch()


c = C()
f(c)
f(c)
C.items = [1]
f(c)
r = C.items
#@value r: unknown
