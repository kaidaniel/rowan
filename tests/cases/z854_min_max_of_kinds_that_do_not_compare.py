"""`min` and `max` compare their arguments: a str with an int, or None with anything, raises
TypeError. (Before, `min('a', 1)` was 'a' or 1 and never raised.)

Expectations were written from the program text before running the engine."""
import sys

try:
    m = min("a", 1)
except TypeError:
    m = "type"
m_seen = m
#@value m_seen: exactly={"type"}

try:
    n = max(None, None)
except TypeError:
    n = "type"
n_seen = n
#@value n_seen: exactly={"type"}

v = "b" if len(sys.argv) > 3 else 2
try:
    k = max(v, 1)
except TypeError:
    k = "type"
k_seen = k
#@value k_seen: includes={2, "type"}

same = max(1, 2.5)
#@value same: exactly={2.5}
