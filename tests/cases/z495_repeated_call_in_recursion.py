# A call made again with the same entry inside a recursive function whose result grows round
# by round: the call that reads the recursion's partial result must see the last round's (a
# call memo keyed by the state alone would replay the first round's).
def depth(xs):
    if not xs:
        return 0
    return 1 + depth(xs[1:])


def wrap(xs):
    return depth(xs)


d = wrap([1, 2, 3])  #@value d: exactly={3}
e = wrap([1, 2, 3])  #@value e: exactly={3}
#@work <= 800
