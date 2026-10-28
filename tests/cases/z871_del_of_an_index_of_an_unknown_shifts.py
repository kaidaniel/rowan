"""`del x[0]` on a value rowan cannot see the class of (`json.loads` of input) that may be a
list: the later items move down, so `x[0]` may still be there afterwards, with another value.
(Before, the index was taken as removed like a dict's key: `x[0]` after it could only raise.)

Expectations were written from the program text before running the engine."""
import json
import sys

text = sys.argv[1] if len(sys.argv) > 1 else "[2, 1]"
x = json.loads(text)
try:
    del x[0]
    first = x[0]
except (KeyError, IndexError, TypeError):
    first = "err"
first_seen = first
#@value first_seen: includes={1, "err"}
