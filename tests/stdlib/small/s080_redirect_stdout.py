# contextlib.redirect_stdout into io.StringIO: a helper's printed output (built from keys it reads) becomes a value

import io
from contextlib import redirect_stdout


def show(rec):
    print(f"{rec['name']}={rec['value']}")        #@ ok name ; possibly value
#@schema param show.rec: always={name,value} only={name,value}


def capture(rec):
    buf = io.StringIO()
    with redirect_stdout(buf):
        show(rec)
    return buf.getvalue()


out = capture({"name": "a", "value": 1})                  #@value out: exactly={"a=1\n"}
out2 = capture({"name": "b", "value": "x", "extra": 0})   #@value out2: exactly={"b=x\n"}
try:
    capture({"name": "c"})                                #@ missing value
except KeyError:
    pass
buf = io.StringIO()
print("x", "y", sep="-", file=buf)
v = buf.getvalue()                                        #@value v: exactly={"x-y\n"}
