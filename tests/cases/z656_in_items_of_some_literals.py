"""`x in xs` when each item of xs is one of some literals: false when x equals none of
them, true when an item is surely x; for a list or set of unknown length, false when x is
none of its elements. A NaN is in a container holding that very object (identity), so
membership of a NaN is never decided by equality. Rowan decided only items that were one
literal each.

Expectations were written from the program text before running the engine."""
import sys

n = len(sys.argv)
a = 1 if n > 2 else 2
pair = [a, a]
no = 0 in pair
#@value no: exactly={False}
maybe = 1 in pair
#@value maybe: types={bool}
yes = 3 in [a, 3]
#@value yes: exactly={True}
s = set(pair)
no_s = 5 in s
#@value no_s: exactly={False}
grown = []
for i in range(n):
    grown.append("x" if i % 2 else "y")
no_g = "z" in grown
#@value no_g: exactly={False}
maybe_g = "x" in grown
#@value maybe_g: types={bool}
nan = float("nan")
holder = [nan, a]
same = nan in holder
#@value same: includes={True}
#@todo value same: exactly={True}  (the very object: identity)
other = float("nan") in [a]
#@value other: exactly={False}
