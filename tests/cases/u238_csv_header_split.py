"""Hypothesis: rows built with `dict(zip(header, line.split(",")))` from literal lines have the
header's keys and known values.

Expectations were written from the program text before running the engine."""

LINES = ["id,name", "1,ann", "2,bob"]
header = LINES[0].split(",")
rows = [dict(zip(header, line.split(","))) for line in LINES[1:]]
r0 = rows[0]                                 #@row r0: present={id,name} definite={id,name} closed
names = {r["name"]: r for r in rows}         #@row names: present={ann,bob} definite={ann,bob} closed
