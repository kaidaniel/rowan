# An f-string of a value code rowan cannot see returned (here a call past the budget) is
# some text: on the path that makes it, `y` is not left as it was.
#@limits budget=0
import sys


def rec(i):
    if i <= 0:
        return "base"
    return rec(i - 1)


n = len(sys.argv)
y = "s"
x = rec(3)
if n % 2 == 0:
    pass
else:
    y = f"{x}-"
p = y                                        #@value p: includes={"s", "base-"}
