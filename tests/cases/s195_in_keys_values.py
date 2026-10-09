"""Hypothesis: membership in `d.keys()` and `d.values()` of a closed dict with known values
is decided.

Expectations were written from the program text before running the engine."""

d = {"a": "x", "b": "y"}
r = {}
if "a" in d.keys():
    r["k"] = 1
if "x" in d.values():
    r["v"] = 1
if "z" in d.values():
    r["z"] = 1
_ = 0                                        #@row r: present={k,v} definite={k,v} closed
