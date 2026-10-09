"""Weakness: a `while` with an unknown test: keys stored in the body are possible.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

st = {"n": 0}
while keep_going():
    st["more"] = True
    st["n"] = st["n"] + 1
_ = 0                                        #@row st: present={n,more} definite={n} closed
_ = 1                                        #@row st: possible={n,more} definite_within={n}
st["more"]                                   #@ sound more
