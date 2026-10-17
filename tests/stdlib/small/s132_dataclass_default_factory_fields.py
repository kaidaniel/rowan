# Two fields of one dataclass with `field(default_factory=list)` get a new list each, per
# instance: appending to one field changes neither the other field nor another instance's.
# rowan's generated __init__ gave both fields one list (one allocation for the two factory
# calls): `n` was 2 and `i.lines` held "x" (found by bench/sitepackages.py, scenario s2:
# acme_billing's Invoice.lines and Invoice.notes). Expectations written from the program
# text and the dataclasses documentation.
from dataclasses import dataclass, field


@dataclass
class Inv:
    number: str
    lines: list = field(default_factory=list)
    notes: list = field(default_factory=list)


i = Inv("a")
i.lines.append(1)
i.notes.append(7)
i.notes.append(8)
n = len(i.lines)
#@value n: exactly={1}
m = len(i.notes)
#@value m: exactly={2}
last = i.lines[-1]
#@value last: exactly={1}
j = Inv("b")
k = len(j.lines) + len(j.notes)
#@value k: exactly={0}
