"""`split`, `rsplit`, `partition` and `rpartition` with an empty separator raise ValueError
("empty separator"); a separator that may be empty may raise it. (Before, `split('')` split
at white space and `partition('')` gave three parts: the handlers never ran.)

Expectations were written from the program text before running the engine."""
import os

try:
    r = "a b".split("")
except ValueError:
    r = "empty"
r_seen = r
#@value r_seen: exactly={"empty"}

try:
    p = "a,b".partition("")
except ValueError:
    p = "empty"
p_seen = p
#@value p_seen: exactly={"empty"}

sep = os.environ.get("SEP", "")
try:
    q = len("a,b".rsplit(sep))
except ValueError:
    q = -1
q_seen = q
#@value q_seen: includes={-1}

ok = "a,b".split(",")
n = len(ok)
#@value n: exactly={2}
