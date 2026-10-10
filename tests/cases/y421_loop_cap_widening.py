"""Hypothesis: a loop whose fixpoint does not converge within the iteration cap (a value
growing by one literal per round) is widened, not cut off: after `while` runs an unknown number
of times, `d` may hold keys longer than the rounds analysed, so no key of x's is definitely
missing. (Reproduction from POSSIBLE_BUGS.md, item 1.)

Expectations were written from the program text before running the engine."""
import sys

s = ""
d = {}
while len(sys.argv) > 5:
    s = s + "x"
    d[s] = 1
k = d
k["x" * 45]                                  #@ sound xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
k["x" * 39]                                  #@ sound xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
_ = 0                                        #@row k: open
