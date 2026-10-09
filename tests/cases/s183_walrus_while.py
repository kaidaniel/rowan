"""Hypothesis: a `while` whose test assigns with a walrus (`(n := n - 1) >= 0`) is decided at
every head when n is known, so the loop's keys are exact.

Expectations were written from the program text before running the engine."""

n = 3
seen = {}
while (n := n - 1) >= 0:
    seen[n] = True
_ = 0                                        #@row seen: present={0,1,2} definite={0,1,2} closed
seen[3]                                      #@ missing 3
