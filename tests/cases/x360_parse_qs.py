"""Weakness: unmodelled libraries. `urllib.parse.parse_qs` returns a dict of data keys.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
from urllib.parse import parse_qs

q = parse_qs("a=1&b=2")
q["a"]                                       #@ sound a
q["zz"]                                      #@ sound zz
