# `x.pop()` on an object rowan knows nothing of (an attribute a constructor past the budget
# set) may raise: a list's IndexError reaches the handler.
#@limits budget=0


class Box:
    def __init__(self, v):
        self.v = v
        self.items = []


o = Box(1)
p = Box("s")
p.v = 2
try:
    p.items.pop()
except IndexError:
    p.v = 0
r = p.v                                      #@value r: includes={0}
