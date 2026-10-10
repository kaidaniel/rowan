# a function of a numpy module that the program calls with literal records: its record reads
# are proven (the extra run with open parameters, for shapes, reports no record verdicts), and
# the shapes follow the call: 2 records of 3 fields, normalised per column, times a 3x5 matrix
import numpy as np


def normalize(x):
    mu = x.mean(axis=0, keepdims=True)
    sd = x.std(axis=0, keepdims=True)
    return (x - mu) / sd


def features(rows):
    arr = np.array([[r["a"], r["b"], r["c"]] for r in rows], dtype=float)  #@ ok a ; ok b ; ok c
    return normalize(arr)


data = [{"a": 1, "b": 2, "c": 3}, {"a": 4, "b": 5, "c": 6}]
f = features(data)
s = f.shape                                      #@value s: exactly={(2, 3)}
h = f @ np.ones((3, 5))
hs = h.shape                                     #@value hs: exactly={(2, 5)}
m = h.max(axis=1).shape                          #@value m: exactly={(2,)}
n = len(h)                                       #@value n: exactly={2}
