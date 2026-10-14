"""y492 with a function rowan cannot see (a module attribute named at run time): code rowan
cannot see, given a record, may overwrite its values and change whatever it reaches through
it, at any depth. (y492's `operator.setitem` is modelled now.)

Expectations were written from the program text before running the engine."""
import operator
import os

touch = getattr(operator, os.environ.get("Z488_FN", "setitem"))
d = {"k": 1, "box": [{"deep": 1}]}
touch(d, "k", 5)
v = d["k"]                                #@value v: unknown
inner = d["box"][0]                       #@row inner: open
