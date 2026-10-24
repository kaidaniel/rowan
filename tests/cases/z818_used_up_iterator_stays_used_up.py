"""An iterator over a list that ran to its end gives nothing more, whatever the list gains; one a
loop may have left early gives the rest from where it stands. A reversed list's iterator gives
nothing once the list is empty."""
import os

c2 = os.environ.get("C2") == "1"

xs = [1, 2, 3]
it = filter(lambda v: v != 2, xs)
for v in it:
    if c2:
        break
xs.append(4)
n = len(list(it))
#@value n: includes={0, 2}
#@todo value n: exactly={0, 2}  (used up on one of the paths joined: the rest not known)

ys = [1, 2, 3]
rt = reversed(ys)
for v in rt:
    if v == 2 or c2:
        break
ys.clear()
m = len(list(rt))
#@value m: exactly={0}
