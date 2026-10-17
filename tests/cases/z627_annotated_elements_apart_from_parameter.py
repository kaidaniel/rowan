"""Elements of a parameter annotated `list[Plain]` (a class that declares no fields) are
instances of the class for any call, as the parameter `q: Plain` is one; they are other
objects than `q`. A store through the elements (`u.n = 5`) is not a store into `q`: for any
call `q.n` is the int its `__init__` was given, unless `q` is among the elements (then 5).

Expectations were written from the program text before running the engine."""


class Plain:
    def __init__(self, n: int):
        self.n = n


def mark(us: list[Plain], q: Plain):
    for u in us:
        u.n = 5
    r = q.n
    #@general r: unknown
    #@todo general r: types={int}  (some function stores `.n`: after any code, any value)
    #@value r: exactly={2, 3, 5}
    return r


a = mark([Plain(1)], Plain(2))
b = mark([Plain(1)], (p := Plain(3)))
c = mark([p], p)
