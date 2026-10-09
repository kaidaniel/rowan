"""Hypothesis: `sub in s` on known strings is decided.

Expectations were written from the program text before running the engine."""


def kind(s):
    out = {}
    if "@" in s:
        out["email"] = True
    if s.startswith("+"):
        out["phone"] = True
    return out


e = kind("a@b")                              #@row e: present={email} definite={email} closed
p = kind("+123")                             #@row p: present={phone} definite={phone} closed
