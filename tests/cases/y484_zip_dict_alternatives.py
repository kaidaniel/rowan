"""`dict(zip(keys, v))` with `v` one of several sequences of known lengths: the keys the shortest
pairs are definite, the others possible (the header/row assumption is for unknown lengths)."""
import sys

FIELDS = ("id", "name", "qty")
v = (1, "a", 3) if len(sys.argv) > 1 else (2, "b")
rec = dict(zip(FIELDS, v))                   #@row rec: present={id,name,qty} definite={id,name} closed
rec["qty"]                                   #@ possibly qty
