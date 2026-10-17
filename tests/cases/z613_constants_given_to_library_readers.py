"""Hypothesis: a module constant the program only gives to library functions that read their
arguments and keep nothing of them (`json.dumps`, `fnmatch.filter`, `math.prod`) is not changed
by them: for any call of a function (its hover), the constant is what the module made, not
`list[?]`. One given to a library call that keeps it (`threading.Thread(args=…)`), or to one
also given the program's function (`json.dumps(…, default=enc)`: the function runs on what it
is given), may change.

Expectations were written from the program text before running the engine."""
import fnmatch
import json
import math
import re
import threading

OPTS = {"indent": 2}
PARTS = ["a", "b"]
PATTERN = ["x"]
COUNT = [3]
KEPT = [1]


WITH_CODE = [{1}]


def dumped():
    return json.dumps(OPTS)


def matching():
    return fnmatch.filter(PARTS, "a*")


def cleaned(text):
    return re.sub(PATTERN[0], "", text)


def product():
    return math.prod(COUNT)


def started(work):
    t = threading.Thread(target=work, args=KEPT)
    t.start()


def enc(o):
    o.add(2)
    return sorted(o)


def encoded():
    return json.dumps(WITH_CODE, default=enc)


def use():
    o = OPTS["indent"]
    #@general o: exactly={2}
    p = PARTS[1]
    #@general p: exactly={"b"}
    q = PATTERN[0]
    #@general q: exactly={"x"}
    c = COUNT[0]
    #@general c: exactly={3}
    k = KEPT[0]
    #@general k: unknown
    w = len(WITH_CODE[0])
    #@general w: includes={1, 2}
    return o, p, q, c, k, w


use()
dumped()
matching()
cleaned("x")
product()
