#@limits budget=0
# A call that does not run (past its budget) of a function that may return its argument
# (`return xs`, `return o if flag else None`): the result may be that very object, so clearing
# it may clear `a` too, and a store into it is one into b.
import sys
def f_ret(xs, v):
    if v in xs:
        return xs
    return [v]
def or_none(o, flag):
    return o if flag else None
class Box:
    pass
n = len(sys.argv)
warm = f_ret([3], 1)
a = [1, 0]
c = f_ret(a, 0)
a.clear()
a.append(5)
c.clear()
k = len(a)
#@value k: includes={0}
#@todo value k: exactly={0}  (the call does not run: a may or may not be the result, 0 | 1)
b = Box()
warm2 = or_none(Box(), n)
f = or_none(b, n)
b.v = 1
f.v = 2
w = b.v
#@value w: includes={2}
class Acc:
    def last(self):
        return 7
def apply(acc):
    return acc.last()
warm3 = apply(Acc())
m = apply(Acc()) + 1
#@value m: includes={8}
