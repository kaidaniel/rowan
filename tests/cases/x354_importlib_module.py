"""Weakness: dynamic imports. A module imported by a name from data is unknown.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import importlib

m = importlib.import_module(name_in)
cfg = m.CONFIG
cfg["key"]                                   #@ sound key
