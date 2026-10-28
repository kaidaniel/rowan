"""`w + 2` where w may be a list, a number or an object rowan cannot see (`json.loads` of
input): the list raises TypeError, the number adds, the unknown may do either. (Before, the
list's part took the other side's elements, which a number has not, and that TypeError ended
the whole path: `except TypeError:` was the only way on, and the sum 2 and the unknown were
lost.)

Expectations were written from the program text before running the engine."""
import json
import sys

n = len(sys.argv)
w = 0
if n > 2:
    w = [1]
if n > 3:
    w = json.loads(sys.argv[1])
try:
    r = w + 2
except TypeError:
    r = "te"
r_seen = r
#@value r_seen: includes={2, "te"}

v = 0
if n > 2:
    v = [1]
try:
    s = v + 2
except TypeError:
    s = "te"
s_seen = s
#@value s_seen: exactly={2, "te"}

u = [1]
if n > 3:
    u = json.loads(sys.argv[1])
try:
    t = u + [2]
except TypeError:
    t = "te"
t_len = len(t) if isinstance(t, list) else t
#@value t_len: includes={2}
