# `vars(b)` where b is one of two instances: the attributes of either; one only the first
# has is there on some paths only.
import sys


class B:
    def __init__(self, v):
        self.v = v
        self.w = 0


class C:
    def __init__(self):
        self.v = "c"


n = len(sys.argv)
b = B("s")
if n == 1:
    b = C()
has_w = "w" in vars(b)                       #@value has_w: types={bool}
has_v = "v" in vars(b)                       #@value has_v: exactly={True}
attrs = vars(b)                              #@row attrs: present={v, w} definite={v} closed
