"""A loop over items not known calls a function that appends to a list made before the loop:
every round may append again, so the list's length after the loop is not known; its first
item, put there before the loop, stays known. (The loop takes a round per possible length, up
to the 16 tracked, each running the callee again: a known cost, no bound set here.)"""
import os


def add(out, x):
    out.append(x)


def collect(xs):
    out = ["head"]
    for x in xs:
        add(out, x)
    first = out[0]
    #@value first: exactly={'head'}
    n = len(out)
    #@value n: types={int}
    return out


items = os.environ.get("Z800", "a,b,c").split(",")
res = collect(items)
# (no #@work bound: one round per possible length, a known cost, see the docstring)
