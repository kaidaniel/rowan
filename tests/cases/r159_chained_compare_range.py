"""Hypothesis: chained comparisons and `in range(...)` on known numbers are decided; on an
unknown number all branches stay possible.

Expectations were written from the program text before running the engine."""


def bucket(n):
    b = {"n": n}
    if 0 <= n < 10:
        b["small"] = True
    elif n in range(10, 100):
        b["mid"] = True
    else:
        b["large"] = True
    return b


s = bucket(5)                                #@row s: present={n,small} definite={n,small} closed
m = bucket(42)                               #@row m: present={n,mid} definite={n,mid} closed
big = bucket(500)                            #@row big: present={n,large} definite={n,large} closed
neg = bucket(-1)                             #@row neg: present={n,large} definite={n,large} closed
u = bucket(n_in)                             #@row u: present={n,small,mid,large} definite={n} closed
m["small"]                                   #@ missing small
