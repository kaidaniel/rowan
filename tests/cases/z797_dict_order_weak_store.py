"""A dict whose insertion order is known but one of whose keys is there on some paths only
("a" before "b", or "b" alone), stored through a name that may be another dict: on the paths
where it lacked "a", "a" now comes last ("b", "a"), so the last key may be either."""
import os

c = os.environ.get("Z797_C", "") == "yes"
d1 = {}
if c:
    d1["a"] = 1
d1["b"] = 2
d2 = {}
t = d1 if os.environ.get("Z797_T", "d1") == "d1" else d2
t["a"] = 3
first = None
last = None
for k in d1:
    if first is None:
        first = k
    last = k
got_first = first
#@value got_first: includes={"b"}
got_last = last
#@value got_last: includes={"a", "b"}
