"""A Series keeps only some of its values once rows are dropped (`head`, `drop`, a `.loc`
row selection with a column) and gains NaN where `reindex` asks for labels it lacks: the
dummy columns made of them are each maybe there (which ones the rows decide). Rowan kept the values as surely there:
`get_dummies` claimed every one.

Expectations were written from the program text and pandas' documented behaviour before
running the engine."""
import pandas as pd

df = pd.DataFrame({"k": ["a", "b", "c"], "v": [1, 2, 3]})
d0 = pd.get_dummies(df["k"])
#@row d0: present={a,b,c} definite={a,b,c} closed
d1 = pd.get_dummies(df["k"].head(1))
#@row d1: present={a,b,c} definite={} closed
#@todo row d1: present={a} definite={a} closed  (the first row's value)
d2 = pd.get_dummies(df["k"].drop([0]))
#@row d2: present={a,b,c} definite={} closed
#@todo row d2: present={b,c} definite={b,c} closed  (label 0 held "a")
d3 = pd.get_dummies(df.loc[df["v"] > 2, "k"])
#@row d3: present={a,b,c} definite={} closed
#@todo row d3: present={c} definite={c} closed  (the rows where v > 2)
d4 = pd.get_dummies(df["k"].reindex([0, 5]))
#@row d4: present={a,b,c} definite={} closed
#@todo row d4: present={a} definite={a} closed  (label 5 is NaN, which get_dummies drops)
d5 = pd.get_dummies(df.loc[:, "k"])
#@row d5: present={a,b,c} definite={a,b,c} closed
