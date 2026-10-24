"""`y.strip` where `y` is 1 or an object rowan cannot see: the int has no `strip` (an
AttributeError there), and the object's method is unknown; calling it gives anything, not a
result made of the int (found by the fuzzer at a fresh seed)."""
import sys

n = len(sys.argv)
y = 1 if n > 2 else type(sys).__name__
try:
    y = y.strip
    y = y()
except AttributeError:
    y = "ae"
r = y
#@value r: includes={"ae", "module"}
