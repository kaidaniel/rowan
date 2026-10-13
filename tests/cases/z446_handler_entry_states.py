"""A handler (or a finally) starts from the state where the exception was raised: a call of
unknown code may raise in the middle of the try body, after stores the body later undoes. The
state before the try joined with the state at its end misses those stores."""
import os
import sys


def stat_missing():
    # unknown library code that raises FileNotFoundError at run time
    return os.stat(sys.argv[0] + ".does-not-exist")


def f():
    d = {"a": 1}
    try:
        d["b"] = 2
        stat_missing()
        del d["b"]
        d["c"] = 3
    except Exception:
        h = dict(d)
        #@row h: present={a,b} definite={a,b} closed
        return h
    return d


r1 = f()
#@row r1: present={a,b,c} definite={a} closed


def risky(d):
    d["x"] = 1
    stat_missing()
    d["y"] = 2


def k():
    d = {}
    try:
        risky(d)
    except OSError:
        r = dict(d)
        #@row r: present={x} definite={x} closed
        return r
    return d


r2 = k()
#@row r2: present={x,y} definite={x} closed


def g():
    st = {"phase": "init"}
    try:
        st["phase"] = "run"
        stat_missing()
        st["phase"] = "done"
    finally:
        last = st["phase"]
        #@value last: exactly={"run", "done"}
    return st


try:
    g()
except OSError:
    pass
