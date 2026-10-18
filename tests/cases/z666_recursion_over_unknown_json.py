# A scan over JSON of unknown shape that calls itself from two places (a dict's values, a
# list's items) and dispatches a tag by an unknown key (flask's TaggedJSONSerializer). Each
# level of the recursion used to be an activation of its own, four deep, each running its
# comprehensions' rounds and its recursion's rounds inside the one above: the work grew as
# (places x rounds)^4. Past one level a call of itself with nothing known but the running
# activation's arguments is that activation's recursive call. Over known data the recursion
# still runs level by level (`known`).
import json
import sys


class Tag:
    key = ""

    def __init__(self, ser):
        self.ser = ser

    def to_python(self, value):
        return value


class TagA(Tag):
    key = " a"

    def to_python(self, value):
        return {k[:-2]: value[k] for k in value}


class TagB(Tag):
    key = " b"

    def to_python(self, value):
        return tuple(value)


class TagC(Tag):
    key = " c"

    def to_python(self, value):
        return str(value)


class Ser:
    def __init__(self):
        self.tags = {}
        for cls in [TagA, TagB, TagC]:
            t = cls(self)
            self.tags[t.key] = t

    def untag(self, value):
        if len(value) != 1:
            return value
        key = next(iter(value))
        if key not in self.tags:
            return value
        return self.tags[key].to_python(value[key])

    def scan(self, value):
        if isinstance(value, dict):
            value = {k: self.scan(v) for k, v in value.items()}
            value = self.untag(value)
        elif isinstance(value, list):
            value = [self.scan(item) for item in value]
        return value

    def loads(self, text):
        return self.scan(json.loads(text))


s = Ser()
known = s.scan({"a": [1, 2]})
#@row known: present={a} definite={a} closed
first = known["a"][0]  #@ ok a
#@value first: exactly={1}
data = s.loads(sys.argv[1] if len(sys.argv) > 1 else '{" b": [1]}')
#@value data: unknown
#@work <= 1000
