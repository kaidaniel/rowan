"""`dd.get(k)` on a defaultdict does not call its factory: a key it lacks gives the default
(None) and is not inserted; only `dd[k]` inserts. (Before, `get` read the item as `dd[k]`
does: a fresh list, never None.)

Expectations were written from the program text before running the engine."""
import json
import sys
from collections import defaultdict

text = sys.argv[1] if len(sys.argv) > 1 else '[["a", 5]]'
d = defaultdict(list)
for k, x in json.loads(text):
    d[k].append(x)
got = d.get("q")
#@value got: includes={None}
plain = defaultdict(list)
missing = plain.get("q")
#@value missing: exactly={None}
size = len(plain)
#@value size: exactly={0}
