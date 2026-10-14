"""Hypothesis: what unknown code may have stored into a record is "anything", and stays so:
a store or a `clear()` through a value that may be such an object does not make every
other value unknown code stored the stored value (a global sink: click's parser state read
as dicts, tuples and options), nor an empty dict (pyparsing's `_trim_arity_call_line[0]`
read "never set" after an unrelated `.clear()`). A value stored there goes to unknown code.

Expectations were written from the program text before running the engine."""

import reprlib


class Box:
    pass


d = {"k": Box(), "m": {"a": 1}}
reprlib.repr(d)                              # unknown code given d: its values may be anything
v = d["k"]
v.tag = "mine"
w = d["m"]
w.clear()

e = {"j": [1, 2], "s": "text"}
reprlib.repr(e)
first = e["j"][0]                            #@ sound 0
#@value first: includes={1}
s = e["s"]                                   #@value s: includes={text}
#@todo value s: exactly={text}
