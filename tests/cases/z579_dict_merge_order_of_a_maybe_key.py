# A dict whose key "k" was deleted on some paths, then merged with a dict holding "k"
# (`d | {...}`, `update`): "k" is first where it was kept, last where it was deleted.
import sys

n = len(sys.argv)
d = {"k": 0}
if n > 2:
    del d["k"]
d["m"] = None
e = d | {"k": 0}
first_e = list(e)[0]                         #@value first_e: includes={"k", "m"}
f = dict(d)
f.update({"k": 0})
first_f = list(f)[0]                         #@value first_f: includes={"k", "m"}
#@todo value first_f: exactly={"k", "m"}  (the keys of a dict in no known order are not taken as literals)
