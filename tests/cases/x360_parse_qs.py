"""Was a weakness (unmodelled parse_qs; floors `sound a`, `sound zz`): parse_qs of a literal is now its dict.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
from urllib.parse import parse_qs

q = parse_qs("a=1&b=2")
q["a"]                                       #@ ok a
q["zz"]                                      #@ missing zz
