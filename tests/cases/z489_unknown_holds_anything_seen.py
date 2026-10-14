"""z465 with a function rowan cannot see (a module attribute named at run time; z465's
`reprlib.repr` is modelled now): what unknown code may have stored into a record is
"anything", and stays so; a store or a `clear()` through a value that may be such an object
changes no other value, and a value stored there goes to unknown code.

Expectations were written from the program text before running the engine."""
import os
import reprlib


class Box:
    pass


peek = getattr(reprlib, os.environ.get("Z489_FN", "repr"))
d = {"k": Box(), "m": {"a": 1}}
peek(d)                                      # unknown code given d: its values may be anything
v = d["k"]
v.tag = "mine"
w = d["m"]
w.clear()

e = {"j": [1, 2], "s": "text"}
peek(e)
first = e["j"][0]                            #@ sound 0
#@value first: includes={1}
s = e["s"]                                   #@value s: includes={text}
