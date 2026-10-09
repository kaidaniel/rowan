"""Hypothesis: a KeyError raised two calls down (a missing literal key) is caught by the
caller's handler; with the key present the normal result is returned.

Expectations were written from the program text before running the engine."""


def need(d, k):
    return d[k]


def soft(d, k):
    try:
        return {"val": need(d, k)}
    except KeyError:
        return {"none": True}


a = soft({"x": 1}, "x")                      #@row a: present={val} definite={val} closed
b = soft({"x": 1}, "y")                      #@row b: present={none} definite={none} closed
