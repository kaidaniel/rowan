"""Hypothesis: a read that may fail raises to a running `finally` (and a `with`'s `__exit__` of the
program's code) although no handler catches its exception: an input's key the input may lack,
a key or attribute stored on some paths only, an attribute of an object rowan cannot see. The
finally starts from the state at the read too; a `return` in it ends the raise there, so the
call goes on with that state. A read that cannot fail (a key stored on every path) adds no
such state. Before, only a handler catching the read's class saw it: `at` below was "done".

Expectations were written from the program text before running the engine."""
import json
import os

ROWS = json.loads(os.environ.get("Z770_ROWS", '[{"n": 1}]'))


def total(rows):
    stage = "start"
    try:
        stage = "reading"
        ns = [r["n"] for r in rows]
        stage = "done"
    finally:
        at = stage
        #@value at: exactly={"reading", "done"}
    return ns


def stored_on_some_paths(flag):
    d = {}
    if flag:
        d["a"] = 1
    stage = 1
    try:
        stage = 2
        v = d["a"]
        stage = 3
    finally:
        s = stage
        #@value s: exactly={2, 3}
    return v


def stored_always():
    d = {"a": 1}
    stage = 1
    try:
        stage = 2
        v = d["a"]
        stage = 3
    finally:
        s2 = stage
        #@value s2: exactly={3}
    return v


class Job:
    def __init__(self, flag):
        if flag:
            self.out = "x"


def attr_on_some_paths(flag):
    j = Job(flag)
    stage = "start"
    try:
        stage = "reading"
        o = j.out
        stage = "done"
    finally:
        s3 = stage
        #@value s3: exactly={"reading", "done"}
    return o


def swallowed(row):
    r = "start"
    try:
        r = "reading"
        row["n"]
        r = "done"
    finally:
        return r


def unknown_attr(obj):
    try:
        v = obj.size
        how = "attr"
    except AttributeError:
        how = "missing"
    return how
    #@value how: exactly={"attr", "missing"}


class Track:
    def __init__(self):
        self.stage = "start"

    def __enter__(self):
        return self

    def __exit__(self, et, ev, tb):
        seen = self.stage
        #@value seen: exactly={"reading", "done"}
        return False


def managed(row):
    with Track() as t:
        t.stage = "reading"
        n = row["n"]
        t.stage = "done"
    return n


total(ROWS)
stored_on_some_paths(True)
stored_on_some_paths(False) if len(ROWS) > 5 else None
stored_always()
attr_on_some_paths(len(ROWS) > 0)
out = swallowed(ROWS[0])
#@value out: exactly={"reading", "done"}
how = unknown_attr(ROWS[0])
managed(ROWS[0])
