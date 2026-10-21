"""Hypothesis: the groups and positions of a match on a text rowan does not know are unknown
strs and ints, and reading them raises nothing a handler of NameError (or of any Exception)
could catch: `m.group(1)`, `m.groups()`, `m.groupdict()`, `m.start()`, `m.span()`.

Expectations were written from the program text before running the engine."""
import os
import re

TEXT = os.environ.get("Z767_TEXT", "v1.2")


def version(t):
    m = re.match(r"v(\d+)\.(\d+)", t)
    if m is None:
        return "none"
    try:
        major = m.group(1)
        parts = m.groups()
        at = m.start()
        span = m.span()
    except NameError:
        return "name"
    return "ok"


r1 = version(TEXT)
#@value r1: exactly={"ok", "none"}


def named(t):
    m = re.search(r"(?P<key>\w+)=(?P<val>\w+)", t)
    if not m:
        return "none"
    try:
        d = m.groupdict()
        k = d["key"]
    except Exception:
        return "failed"
    return "ok"


r2 = named(TEXT)
#@value r2: exactly={"ok", "none"}
