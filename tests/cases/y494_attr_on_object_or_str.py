"""A value that may be a str or an object of a user class: `.replace` exists on the str, so the
read fails on some runs only. Each object's verdict was taken as the read's and the str part
ignored: `missing replace` (rich's markup parser, where tuple positions were joined)."""
import sys


class Tag:
    def __init__(self, name):
        self.name = name


vals = []
for p in sys.argv[1:] or ["-a", "b"]:
    vals.append(p if p.startswith("-") else Tag(p))
for v in vals:
    try:
        v.replace("x", "y")                      #@ possibly replace
    except AttributeError:
        pass
