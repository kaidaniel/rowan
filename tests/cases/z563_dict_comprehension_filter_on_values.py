# A dict comprehension whose filter tests the value (`if v != 0`), over items whose values
# differ by path: a key the filter may drop is at most maybe there.
import sys

n = len(sys.argv)
e = {"k": 0}
if n % 2 == 0:
    e["m"] = 1
kept = {k: v for k, v in e.items() if v != 0}
has_k = "k" in kept                          #@value has_k: includes={False}
#@todo value has_k: exactly={False}  (the items of a dict with a key on some paths are joined, not taken key by key)
got = kept.get("m", -1)                      #@value got: includes={1, -1}
#@todo value got: exactly={1, -1}
