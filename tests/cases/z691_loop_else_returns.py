# A loop's `else:` that returns, beside a `break`: the function returns the else's value on
# the runs that end the loop without breaking (countdown(n) with n under 6, search for 9).
import sys
def countdown(k):
    while k > 0:
        k -= 1
        if k == 5:
            break
    else:
        return 0
    return 1
def search(xs, t):
    for i, w in enumerate(xs):
        if w == t:
            break
    else:
        return -1
    return i
n = len(sys.argv)
a = countdown(n)
#@value a: exactly={0, 1}
xs = [1, 2, 3]
for _i in range(n):
    xs.append(0)
b = search(xs, 9)
#@value b: exactly={-1}
c = search(xs, 2)
#@value c: includes={1}
#@todo value c: exactly={1}  (a list of unknown length: the loop is a fixpoint, i any int)
