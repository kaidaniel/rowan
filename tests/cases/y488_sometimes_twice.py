"""A key a function reads only when it did not store it, called twice on one input: with
`argv > 2` the first call stores it, so the input need not have it (a second access needed
on some paths must not make the requirement `always`)."""
import json
import sys


def total(d, flag):
    if flag:
        d["total"] = 0
    return d["total"]


for line in sys.stdin:
    d = json.loads(line)
    total(d, len(sys.argv) > 2)
    total(d, len(sys.argv) > 3)
#@schema json.loads@L15: sometimes={total}
