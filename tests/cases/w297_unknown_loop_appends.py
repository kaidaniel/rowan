"""Hypothesis: dicts appended in a loop of unknown length all have the literal's keys.

Expectations were written from the program text before running the engine."""

rows = []
for line in lines_in:
    rows.append({"a": line, "n": 1})
rows[0]["a"]                                 #@ ok a
rows[0]["b"]                                 #@ missing b
