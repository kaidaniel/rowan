"""Hypothesis: the KeyError raised by `d[k]` carries the key: `e.args[0]` in the handler is
the missing key.

Expectations were written from the program text before running the engine."""

d = {"a": 1}
missing = {}
for k in ["a", "b"]:
    try:
        d[k]
    except KeyError as e:
        missing[e.args[0]] = True
_ = 0                                        #@row missing: present={b} definite={b} closed
