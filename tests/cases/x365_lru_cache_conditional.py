"""Weakness: model corner. `lru_cache` results under a condition: a store on the cached result
made in one branch is only possibly seen later.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
from functools import lru_cache


@lru_cache(maxsize=None)
def load(name):
    return {"name": name}


if cond:
    a = load("x")
    a["seen"] = True
b = load("x")
# Corrected on re-reading: the row check goes before the reads (a read that does not raise
# proves the key).
_ = 0                                        #@row b: possible={name,seen} definite_within={name}
b["seen"]                                    #@ possibly seen
b["seen"]                                    #@ sound seen
