# A dict comprehension whose filter tests the value (`if v != 0`), over items whose values
# differ by path: a key the filter may drop is at most maybe there.
import sys

n = len(sys.argv)
e = {"k": 0}
if n % 2 == 0:
    e["m"] = 1
kept = {k: v for k, v in e.items() if v != 0}
has_k = "k" in kept                          #@value has_k: exactly={False}
got = kept.get("m", -1)                      #@value got: exactly={1, -1}
