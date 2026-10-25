"""`del xs[i]` of an index outside the list raises IndexError, as reading it does; a slice never
raises. (Before, the `del` of a missing position went on as if it removed nothing.)

Expectations were written from the program text before running the engine."""
import sys

xs = []
try:
    del xs[0]
    a = "deleted"
except IndexError:
    a = "index"
got_a = a
#@value got_a: exactly={"index"}

ys = [1, 2]
try:
    del ys[-3]
    b = "deleted"
except IndexError:
    b = "index"
got_b = b
#@value got_b: exactly={"index"}

zs = [1, 2, 3]
del zs[1:]
n = len(zs)
#@value n: exactly={1}

ws = list(range(len(sys.argv)))
try:
    del ws[1]
    c = "deleted"
except IndexError:
    c = "index"
got_c = c
#@value got_c: includes={"index"}
