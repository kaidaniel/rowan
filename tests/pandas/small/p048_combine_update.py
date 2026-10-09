# combine_first / combine: the union of both frames' columns; update: in place on the caller's columns only (the other's extra columns are ignored), returns None
import pandas as pd


def pick(s1, s2):
    return s1.fillna(s2)


def combined(a, b):
    c = a.combine_first(b)
    #@row c: present={k,x,y,z} definite={k,x,y} closed
    return c["y"]                                  #@ ok y


def combined_fn(a, b):
    c = a.combine(b, pick)
    #@row c: present={k,x,y,z} definite={k,x,y} closed
    return c


def updated(a, b):
    res = a.update(b)
    #@value res: exactly={None}
    #@row a: present={k,x} definite={k,x} absent={y} closed
    a["y"]                                         #@ missing y


if __name__ == "__main__":
    pairs = [(pd.DataFrame({"k": [1], "x": [None]}), pd.DataFrame({"k": [1], "y": [2]})),
             (pd.DataFrame({"k": [1], "x": [2.0], "z": [3]}), pd.DataFrame({"x": [4.0], "y": [5]}))]
    for a, b in pairs:
        combined(a, b)
        combined_fn(a, b)
    for a, b in ((pd.DataFrame({"k": [1], "x": [2]}), pd.DataFrame({"x": [9], "y": [3]})),
                 (pd.DataFrame({"k": [1], "x": [2]}), pd.DataFrame({"y": [3]}))):
        try:
            updated(a, b)
        except KeyError:
            pass
