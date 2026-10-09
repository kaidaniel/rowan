"""Hypothesis: a key first stored inside a loop of unknown length is possible after it (the
loop may not run); a key present before the loop stays definite.

Expectations were written from the program text before running the engine."""

out = {}
for x in xs_in:
    out["last"] = x
out["last"]                                  #@ possibly last
seen = {"init": 1}
for x in xs_in:
    seen["init"] = x
seen["init"]                                 #@ ok init
