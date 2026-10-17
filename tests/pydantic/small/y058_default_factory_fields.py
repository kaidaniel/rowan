"""Two fields of one pydantic model with `Field(default_factory=list)` (and two private
attributes with `PrivateAttr(default_factory=dict)`) get a new container each, per instance:
appending to one field changes neither the other field nor another instance's. A plain
mutable default (`tags: list = []`) is copied per instance and per field too.

Expectations were written from the program text and pydantic's documented behaviour before
running the engine."""
from pydantic import BaseModel, Field, PrivateAttr


class Inv(BaseModel):
    number: str
    lines: list = Field(default_factory=list)
    notes: list = Field(default_factory=list)
    tags: list = []
    flags: list = []
    _cache: dict = PrivateAttr(default_factory=dict)
    _seen: dict = PrivateAttr(default_factory=dict)


i = Inv(number="a")
i.lines.append(1)
i.notes.append(7)
i.notes.append(8)
i.tags.append("t")
i._cache["k"] = 1
n = len(i.lines)
#@value n: exactly={1}
m = len(i.notes)
#@value m: exactly={2}
f = len(i.flags)
#@value f: exactly={0}
last = i.lines[-1]
#@value last: exactly={1}
s = len(i._seen)
#@value s: exactly={0}
j = Inv(number="b")
k = len(j.lines) + len(j.notes) + len(j.tags)
#@value k: exactly={0}
