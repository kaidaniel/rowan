"""Hypothesis: a `finally` runs after the return value was computed: it changes the returned
dict (same object) but not a rebound local; on a raising path it runs too. Precise: build()
returns {status, body, closed}; compute() returns 1; strip() returns d without "secret"; after
risky() raised KeyError, d has {a, touched} with touched "b"."""
import json
import os


def build(log):
    out = {"status": "start"}
    try:
        out["body"] = 1
        return out
    finally:
        out["closed"] = True
        log.append("fin")


def compute():
    x = 1
    try:
        return x
    finally:
        x = 2


def strip(d):
    try:
        return d
    finally:
        d.pop("secret", None)


def risky(d, k):
    try:
        return d[k]
    finally:
        d["touched"] = k


if __name__ == "__main__":
    log = []
    b = build(log)
    #@row b: present={status,body,closed} definite={status,body,closed} closed
    nlog = len(log)                              #@value nlog: exactly={1}
    x = compute()                                #@value x: exactly={1}
    s = strip({"a": 1, "secret": 2})
    #@row s: present={a} definite={a} absent={secret} closed
    d = {"a": 1}
    try:
        got = risky(d, "b")
    except KeyError:
        got = None
    _ = 0                                        #@value got: exactly={None}
    _ = 0                                        #@row d: present={a,touched} definite={a,touched} closed
    tk = d["touched"]                            #@value tk: exactly={"b"}
    u = json.loads(os.environ.get("Z435_U", '{"secret": 0, "z": 1}'))
    su = strip(u)
    #@row su: absent={secret} open
