"""Hypothesis: `vars(obj)` and `obj.__dict__` are the instance's attribute dict itself, so a
store through them adds an attribute.

Expectations were written from the program text before running the engine."""


class O:
    def __init__(self):
        self.a = 1


o = O()
v = vars(o)
v["z"] = 2
o.z                                          #@ ok z
o.__dict__["w"] = 3
o.w                                          #@ ok w
o.q                                          #@ missing q
