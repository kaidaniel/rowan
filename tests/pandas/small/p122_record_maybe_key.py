"""A record that gets a key on some paths only: a frame made from it has that column on those
paths (`possibly` for reads), the record's sure keys are definite columns."""
import os

import pandas as pd


def record(flag):
    d = {"a": 1}
    if flag:
        d["b"] = 2
    return d


def frame(flag):
    df = pd.DataFrame([record(flag)])
    #@row df: present={a,b} definite={a} closed
    a = df["a"]                                  #@ ok a
    try:
        b = df["b"]                              #@ possibly b
    except KeyError:
        pass
    return df


if __name__ == "__main__":
    for flag in (os.environ.get("P122_FLAG", "1") == "1", False):
        frame(flag)
    both = pd.DataFrame([record(True), record(False)])
    #@row both: present={a,b} definite={a,b} closed
