"""Weakness: dynamic code. `eval` of a literal string is that literal; of anything else, a
value nothing is known about.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = eval("{'a': 1}")
d["a"]                                       #@ ok a
# Corrected on re-reading: the code string is a literal, so the dict is known and "b" really is
# missing (KeyError).
d["b"]                                       #@ missing b
e = eval(code_in)
e["k"]                                       #@ sound k
