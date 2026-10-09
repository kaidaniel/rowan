"""Weakness: a store through an unknown index of a list of two dicts: possible on each.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

rows = [{"id": 1}, {"id": 2}]
rows[i_in]["seen"] = True
rows[0]["seen"]                              #@ possibly seen
rows[1]["seen"]                              #@ sound seen
rows[0]["id"]                                #@ ok id
