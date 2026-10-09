"""Hypothesis: `d.get(k, DEFAULT)` returns the DEFAULT object itself when k is missing (so a
store through the result changes DEFAULT, not d), and the stored value when k is present.

Expectations were written from the program text before running the engine."""

DEFAULT = {"d": 1}
cfg = {}
got = cfg.get("x", DEFAULT)
got["added"] = True
_ = 0                                        #@row DEFAULT: present={d,added} definite={d,added} closed
_ = 1                                        #@row cfg: present={} definite={} closed
present = {"x": {"own": 1}}
got2 = present.get("x", DEFAULT)
got2["mine"] = 1
_ = 2                                        #@row DEFAULT: present={d,added} definite={d,added} closed
inner = present["x"]                         #@row inner: present={own,mine} definite={own,mine} closed
