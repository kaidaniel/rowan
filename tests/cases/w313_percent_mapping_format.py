"""Hypothesis: `"%(user)s@%(host)s" % rec` reads user and host from rec.

Expectations were written from the program text before running the engine."""
import json

rec = json.loads(text_in)
s = "%(user)s@%(host)s" % rec
#@schema json.loads@L6: always={user,host}
