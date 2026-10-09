"""Weakness: unmodelled libraries. sqlite3 rows have data columns: no access is definitely
missing.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import sqlite3

conn = sqlite3.connect(":memory:")
conn.row_factory = sqlite3.Row
r = conn.execute("select 1 as id").fetchone()
r["id"]                                      #@ sound id
