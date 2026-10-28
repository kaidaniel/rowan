"""A name bound to a number compared with an unknown number on its right (`y == x`, x =
len(argv)): the test may be true or false. (Before, with the number on the left, the true
branch was unreachable: `if y == x: found = y` never found anything, and a loop never broke
there; `if x == y` and `if 1 == x` were right.)

Expectations were written from the program text before running the engine."""
import sys

x = len(sys.argv)
y = 1
found = 0
if y == x:
    found = y
found_seen = found
#@value found_seen: exactly={0, 1}

G = 0
for i in [1, 2]:
    if i == x:
        break
    G = G * 10 + i
trace = G
#@value trace: exactly={0, 1, 12}

k = 2
if k != x:
    other = "ne"
else:
    other = "eq"
other_seen = other
#@value other_seen: exactly={"ne", "eq"}

# the unknown beside a known number (a hint): it may still equal y
z = 0 if len(sys.argv) > 5 else len(sys.argv)
if y == z:
    hit = "yes"
else:
    hit = "no"
hit_seen = hit
#@value hit_seen: exactly={"no", "yes"}
