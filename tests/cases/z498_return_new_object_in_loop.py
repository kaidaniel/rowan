# An object made in a loop iteration and returned from inside the loop: the caller gets the
# object as its __init__ left it (the loop going on to later iterations does not lose its
# attributes).
import sys


class H:
    def __init__(self):
        self.c = 1

    def norm(self):
        x = self.c  #@ ok c
        self.c = 2
        return x


def get(xs):
    for v in xs:
        if v:
            return H()
    return None


def first(xs):
    for v in xs:
        return H()


h = get(sys.argv)
n = None
if h is not None:
    n = h.norm()  #@value n: exactly={1}
#@row h: present={c} definite={c} closed
g = first(sys.argv)
#@value g: includes={None}
