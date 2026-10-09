"""Hypothesis: a `while` with an unknown test may run zero times: its stores are possible,
earlier keys stay definite.

Expectations were written from the program text before running the engine."""

st = {"n": 0}
while more_in():
    st["n"] += 1
    st["looped"] = True
st["looped"]                                 #@ possibly looped
st["n"]                                      #@ ok n
