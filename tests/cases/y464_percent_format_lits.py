"""`"k%d" % k` with `k` one of several ints is one of several strings `"k0"`, `"k1"`: each value is
formatted. The conversion to text handled a single literal only: with two, the values stayed ints
and the result was `0 | 1` (dict keys `0?`, `1?` instead of `"k0"?`, `"k1"?`)."""
import sys

k = 0
if sys.argv:
    k = 1
s = "k%d" % k  #@value s: exactly={"k0","k1"}
t = "%s-%s" % ("a", k)  #@value t: exactly={"a-0","a-1"}
d = {s: 1}
print(d)  #@row d: present={k0,k1} definite={} closed
