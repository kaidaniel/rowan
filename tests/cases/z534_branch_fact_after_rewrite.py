# A key stored under a test is there when the same test holds again later (`if c: d["k"] = 1`
# ... `if c: d["k"]`), but only while nothing else changed the dict since: another branch that
# stores or deletes the key, or a store through a name that may hold the dict, ends the fact
# even when the dict's keys look the same as before.
import os


def unchanged(c):
    d = {}
    if c:
        d["k"] = 1
    if c:
        print(d["k"])  #@ ok k
    if not c:
        print(d["k"])  #@ missing k


def stored_again(c, other):
    # with `other` and not `c`, the key is there
    e = {}
    if c:
        e["k"] = 1
    if other:
        e["k"] = 2
    if not c:
        print(e["k"])  #@ possibly k
    if c:
        print(e["k"])  #@ sound k
        #@todo ok k  (stored by the first branch, and the second keeps it; the fact ends at the second store)


def deleted(c, other):
    # with `c` and `other`, the key is gone
    f = {}
    if c:
        f["k"] = 1
    if other:
        f.pop("k", None)
    if c:
        print(f["k"])  #@ possibly k


def through_alias(c, pick):
    # a store through a name that holds f2 or another dict
    f2 = {}
    g = {}
    if c:
        f2["k"] = 1
    h = f2 if pick else g
    h["k"] = 3
    if not c:
        print(f2["k"])  #@ possibly k


def same_test_between(c):
    # a branch on the same test in between: the fact holds on
    m = {}
    if c:
        m["k"] = 1
    if c:
        print("again")
    if c:
        print(m["k"])  #@ ok k


# every combination of the flags, which rowan cannot tell apart (they come from the environment)
for combo in os.environ.get("ROWAN_CASE_COMBOS", "00 01 10 11").split():
    a, b = combo[0] == "1", combo[1] == "1"
    for fn in (unchanged, same_test_between):
        try:
            fn(a)
        except KeyError:
            pass
    for fn in (stored_again, deleted, through_alias):
        try:
            fn(a, b)
        except KeyError:
            pass
