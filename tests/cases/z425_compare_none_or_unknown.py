# a name holding None or an unknown value: == / != / in against literals keep both branches
import json
import sys

v = json.loads(sys.argv[1]) if len(sys.argv) > 2 else None
if v != "k":
    a = 1
else:
    b = 2                                            #@value b: exactly={2}
_ = 0  #@value v: includes={None}


def admin(x):
    if x != "k":
        raise ValueError("no")
    return "ann"


try:
    r = admin(v)                                     #@value r: exactly={ann}
except ValueError:
    r = None
after = 1                                            #@value after: exactly={1}
if v in ("a", "b"):
    w = v                                            #@value w: exactly={a,b}
