"""Hypothesis: a key the program adds to an input record is no requirement; `del rec[k]` needs
k to be there (it raises otherwise), and after it k is missing.

Expectations were written from the program text before running the engine."""
import json

rec = json.loads(text_in)
rec["new"] = 1
rec["new"]                                   #@ ok new
del rec["gone"]
#@schema json.loads@L7: always={gone} only={gone}
rec["gone"]                                  #@ missing gone
