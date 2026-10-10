"""Weakness: model corner. A regex that does not match gives None; a guarded `groupdict` read
is never "definitely missing" (L12 was `ok a`; the match is now computed, None, so the body never runs).

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import re

m = re.match(r"(?P<a>x)", "y")
if m:
    gd = m.groupdict()
    gd["a"]                                  #@ none a
    gd["a"]                                  #@ sound a
