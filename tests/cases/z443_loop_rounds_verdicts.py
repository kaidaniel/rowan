# A loop over items not known runs to a fixpoint: its first round sees the state before the
# loop only (the key not stored yet, the name not bound yet), later rounds what earlier
# iterations did. What an access says is what it says at the fixpoint: here a key and a name
# set by an earlier iteration (an error only when the first matching item comes first).
import sys


def keys(items):
    d = {}
    for x in items:
        if x:
            v = d["k"]  #@ possibly k
        d["k"] = 1
    return d


def names(items):
    for x in items:
        if x:
            v = c  #@ possibly c
        c = 1


def tree(entries):
    out = ""
    for entry in entries:
        if isinstance(entry, tuple):
            name, bases = entry
            out = out + name
        elif isinstance(entry, list):
            out = out + tree(entry) + name  #@ possibly name
    return out


d = {}
for arg in sys.argv:
    if arg == "--again":
        print(d["seen"])  #@ possibly seen
    d["seen"] = True
