"""Weakness: a loop of unknown length that only re-stores a key the dict already has keeps it
definite.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {"last": None}
for x in xs_in:
    d["last"] = x
d["last"]                                    #@ ok last
_ = 0                                        #@row d: present={last} definite={last} closed
