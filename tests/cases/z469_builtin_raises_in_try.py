"""Hypothesis: builtins and container methods that rowan took for calls that cannot raise do
raise on values of the wrong type and on empty containers, and a handler for the exception
runs. `[].pop()` (IndexError) and an attribute of None (AttributeError) fail on every run;
`sorted` of an int and a str, `len` of an int, `",".join` of ints, `hash` of a list and `sum`
of strs (TypeError) do too, though rowan may only know that they may. A TypeError is not
caught by a handler of another class.

Expectations were written from the program text before running the engine."""


def pop_last(xs):
    try:
        v = xs.pop()
    except IndexError:
        return {"empty": True}
    return {"last": v}


r1 = pop_last([])
e1 = r1["empty"]                 #@ ok empty


def upper(s):
    try:
        v = s.upper()
    except AttributeError:
        return {"none": True}
    return {"text": v}


r2 = upper(None)
e2 = r2["none"]                  #@ ok none


def sort_all(xs):
    try:
        v = sorted(xs)
    except TypeError:
        return {"mixed": True}
    return {"sorted": v}


r3 = sort_all([1, "a"])
e3 = r3["mixed"]                 #@ sound mixed
#@todo ok mixed  (sorting an int and a str always raises)


def size(x):
    try:
        n = len(x)
    except TypeError:
        return {"no_len": True}
    return {"n": n}


r4 = size(5)
e4 = r4["no_len"]                #@ sound no_len
#@todo ok no_len  (an int has no len)


def joined(items):
    try:
        v = ",".join(items)
    except TypeError:
        return {"not_str": True}
    return {"text": v}


r5 = joined([1, 2])
e5 = r5["not_str"]               #@ sound not_str
#@todo ok not_str  (join of ints always raises)


def hashed(x):
    try:
        v = hash(x)
    except Exception:
        return {"unhashable": True}
    return {"hash": v}


r6 = hashed([])
e6 = r6["unhashable"]            #@ sound unhashable
#@todo ok unhashable  (a list is unhashable)


def total(xs):
    try:
        v = sum(xs)
    except TypeError:
        return {"bad": True}
    return {"total": v}


r7 = total(["a"])
e7 = r7["bad"]                   #@ sound bad
#@todo ok bad  (0 + "a" raises)


def count(xs):
    try:
        n = len(xs)
    except KeyError:
        return {"never": True}
    return {"n": n}


r8 = count([1, 2])
n8 = r8["n"]                     #@ ok n
#@value n8: exactly={2}
