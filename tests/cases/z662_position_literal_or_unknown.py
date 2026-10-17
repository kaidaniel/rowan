# A position that is a literal on one path and an unknown (an item of parsed JSON) on the other:
# a list's or tuple's read at it may be any item. python z662.py → i is 1 (len(argv)); z662.py x y
# → i is 0. Companion of z659 / z661 (keys and arguments).
import sys
import json
n = len(sys.argv)
i = 0 if n > 2 else json.loads("[" + str(n) + "]")[0]
xs = [10, 20, 30]
p1 = xs[i]
#@value p1: includes={10, 20}
t = (10, 20, 30)
p2 = t[i]
#@value p2: includes={10, 20}
p3 = "abc"[i]
#@value p3: includes={'a', 'b'}
ys = [1, 2, 3]
ys[i] = 9
p4 = ys[1]
#@value p4: includes={2, 9}
zs = [1, 2, 3]
del zs[i]
p5 = zs[0]
#@value p5: includes={1, 2}
ws = [1, 2, 3]
ws.pop(i)
p6 = ws[0]
#@value p6: includes={1, 2}
vs = [1, 2, 3]
vs.insert(i, 7)
p7 = vs[1]
#@value p7: includes={1, 7}
p8 = xs[i:]
p9 = len(p8)
#@value p9: includes={2, 3}
