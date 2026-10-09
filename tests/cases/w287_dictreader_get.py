"""Hypothesis: in a loop over csv.DictReader rows, `row["amount"]` is required and
`row.get("note", "")` is not.

Expectations were written from the program text before running the engine."""
import csv

with open("f.csv") as fh:
    for row in csv.DictReader(fh):
        total = row["amount"]                #@ input amount
        note = row.get("note", "")           #@ none note
#@schema csv.DictReader@L8: always={amount} only={amount}
