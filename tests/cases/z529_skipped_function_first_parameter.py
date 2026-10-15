"""A function's first parameter is a receiver only for a method of a class: a plain function
past its budget that stores into, or calls a method on, its first parameter changes what it
was given (here its default).

Expectations were written from the program text before running the engine."""
#@limits budget=1

LOG = []
BOX = {"n": 0}


class Holder:
    pass


H = Holder()


def note(items=LOG, v=0):
    a = 1
    items.append(v)


def mark(obj=H, v=0):
    a = 1
    obj.seen = v


note(v=1)
note(v=2)
ln = len(LOG)  #@value ln: includes={2}
mark(v=1)
mark(v=2)
seen = H.seen  #@value seen: includes={2}
