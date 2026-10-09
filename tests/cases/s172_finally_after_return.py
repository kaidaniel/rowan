"""Hypothesis: a `finally` runs after a `return` in the try body, so its stores are visible in
the returned object; keys stored after the return on that path are not.

Expectations were written from the program text before running the engine."""


def build():
    d = {"a": 1}
    try:
        return d
    finally:
        d["f"] = True


def build2(flag):
    d = {}
    try:
        if flag:
            return d
        d["no"] = 1
    finally:
        d["fin"] = 1
    d["after"] = 1
    return d


r = build()                                  #@row r: present={a,f} definite={a,f} closed
x = build2(True)                             #@row x: present={fin} definite={fin} closed
y = build2(False)                            #@row y: present={no,fin,after} definite={no,fin,after} closed
x["after"]                                   #@ missing after
