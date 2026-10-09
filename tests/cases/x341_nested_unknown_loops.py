"""Weakness: nested loops of unknown length writing different keys: both possible.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {}
for a in as_in:
    d["outer"] = a
    for b in bs_in:
        d["inner"] = b
_ = 0                                        #@row d: present={outer,inner} definite={} closed
_ = 1                                        #@row d: possible={outer,inner} definite_within={}
