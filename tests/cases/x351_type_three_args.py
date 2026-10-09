"""Weakness: dynamic classes. `type(name, bases, ns)` makes a class whose instances have the
namespace's attributes.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

C = type("C", (), {"x": 1})
c = C()
# Corrected on re-reading: a class attribute read gives no verdict (as for any class, see
# `class C: x = 1; C().x`), so "none" is the precise answer; an unset attribute is missing.
c.x                                          #@ none x
c.x                                          #@ sound x
c.y                                          #@ missing y
