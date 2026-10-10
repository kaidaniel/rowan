"""Hypothesis: `try: c["primary"] except KeyError: c["fallback"]` requires only the fallback,
and only sometimes.

Expectations were written from the program text before running the engine."""
import json

c = json.loads(text_in)
try:
    v = c["primary"]                         #@ none primary
except KeyError:
    v = c["fallback"]                        #@ input fallback
#@schema json.loads@L7: always={primary} sometimes={fallback} only={primary,fallback}
# (L12 corrected: primary added — its read raises a (caught) KeyError when it is missing, which still
#  counts as required, the runtime oracle's rule)
