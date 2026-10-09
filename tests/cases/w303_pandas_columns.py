"""Hypothesis: columns read from a csv frame are required; a column the program assigns is
not, and reading it afterwards is fine.

Expectations were written from the program text before running the engine."""
import pandas as pd

df = pd.read_csv("o.csv")
df["total"] = df["qty"] * df["price"]
by = df.groupby("region")["total"].sum()
df["total"]                                  #@ ok total
#@schema read_csv@L7: always={qty,price,region} only={qty,price,region}
