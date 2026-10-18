# d has "b" and "a" on one path only (put there by an update), so after "m" is put back at the
# end, a second update of "b" and "a" in an order not known puts them after "m" on the other
# path: the last key is "m" (n == 1) or "a" / "b" (else).
import sys
from collections import OrderedDict
n = len(sys.argv)
d = {"k": 0, "m": 1}
od = OrderedDict(b=2, a=1)
if n == 1:
    d.update(od)
    od.move_to_end("b")
if "m" in d:
    del d["m"]
    d["m"] = 2
d.update(od)
last = list(d)[-1]
#@value last: includes={"m", "a"}
first = list(d)[0]
#@value first: includes={"k"}
#@todo value first: exactly={"k"}  (an order known in part is not kept: a known first key)
