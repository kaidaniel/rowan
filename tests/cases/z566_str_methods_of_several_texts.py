# A str method computed on one of several texts gives each text's result (a list for each:
# `s.split(".")` of "3" | "a.b"); `lstrip()` is not `strip()`.
import sys

n = len(sys.argv)
s = "3"
if n > 2:
    s = "a.b"
parts = s.split(".")
last = parts[-1]                             #@value last: exactly={"3", "b"}
first = parts[0]                             #@value first: exactly={"3", "a"}

padded = " ab ".lstrip()                     #@value padded: exactly={"ab "}
