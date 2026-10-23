"""A generator expression built over the previous one at the same place (`g = (... for r in g)`
in a loop: one object for all of them) given to a call that does not run: its items are taken
once, not again for itself without end (rowan overflowed its stack). (Found by the held-out
corpus: bottle.py, whose analysis crashed once budgets ran out.)"""
#@limits budget=0


def total(xs):
    return sum(xs)


def run(rows, n):
    g = iter(rows)
    for _ in range(n):
        g = (r * 2 for r in g)
    return total(g)


x = run([1, 2, 3], 4)  #@value x: includes={96}
y = run([1, 2], len(str(x)))  #@value y: includes={12}
