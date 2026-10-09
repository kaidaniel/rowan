"""Weakness: a store through a name holding one of two dicts: the key is possible on each,
definite on neither.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

a = {"a": 1}
b = {"b": 1}
t = a if cond else b
t["x"] = 1
# Corrected on re-reading: the row check goes before the reads, since a read that does not
# raise proves the key from then on.
_ = 0                                        #@row a: possible={a,x} definite_within={a}
a["x"]                                       #@ possibly x
b["x"]                                       #@ possibly x
t["x"]                                       #@ ok x
