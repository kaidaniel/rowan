"""`'%c' % n` of an int outside range(0x110000) raises OverflowError, not TypeError; `'%d'` of an
infinite float raises OverflowError and of a NaN ValueError. (Before, the first was claimed
to raise TypeError, and `'%d' % float(...)` of a float not known only TypeError.)

Expectations were written from the program text before running the engine."""
import os

try:
    a = "%c" % (1234567,)
except OverflowError:
    a = "overflow"
except TypeError:
    a = "type"
a_seen = a
#@value a_seen: exactly={"overflow"}

try:
    b = "%c" % (-1,)
except OverflowError:
    b = "overflow"
b_seen = b
#@value b_seen: exactly={"overflow"}

x = float(os.environ.get("X", "inf"))
try:
    d = "%d" % (x,)
except OverflowError:
    d = "overflow"
except ValueError:
    d = "nan"
d_seen = d
#@value d_seen: includes={"overflow"}

ok = "%c" % (65,)
#@value ok: exactly={"A"}
