"""`int()` and `float()` of a text parse as CPython does: digits of any script (`'٣4'` is 34),
underscores only between digits (`'1__0'` raises ValueError), ints past 64 bits. (Before, the
text was trimmed of Rust's white space and stripped of every underscore: `'٣4'` surely raised,
`'1__0'` was 10 and `'99999999999999999999'` raised ValueError.)

Expectations were written from the program text before running the engine."""

try:
    a = int("٣4")
except ValueError:
    a = -1
a_seen = a
#@value a_seen: includes={34}

try:
    b = int("1__0")
except ValueError:
    b = -1
b_seen = b
#@value b_seen: includes={-1}

try:
    c = int("99999999999999999999") > 0
except ValueError:
    c = "error"
c_seen = c
#@value c_seen: includes={True}

try:
    d = float("٣.5")
except ValueError:
    d = -1.0
d_seen = d
#@value d_seen: includes={3.5}

try:
    e = int("abc")
except ValueError:
    e = -1
e_seen = e
#@value e_seen: exactly={-1}

f = int(" 12 ")
#@value f: exactly={12}
