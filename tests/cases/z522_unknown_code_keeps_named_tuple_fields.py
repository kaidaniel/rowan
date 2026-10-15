"""Unknown code given an instance of a NamedTuple class cannot change its fields (the class
has no instance dict, its fields are read-only); what the fields hold may change.

Expectations were written from the program text before running the engine."""
import os
import reprlib
from typing import NamedTuple

peek = getattr(reprlib, os.environ.get("Z522_FN", "repr"))


class Seg(NamedTuple):
    text: str
    meta: dict


s = Seg("a", {"k": 1})
peek(s)
t = s.text  #@ ok text
#@value t: exactly={a}
m = s.meta  #@ ok meta
k = m["k"]  #@ sound k
