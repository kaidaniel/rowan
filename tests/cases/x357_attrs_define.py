"""Weakness: unmodelled libraries. An `attrs` class has its declared fields.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import attrs


@attrs.define
class P:
    x: int
    y: int = 0


p = P(1)
p.x                                          #@ ok x
p.y                                          #@ sound y
