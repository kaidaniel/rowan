"""An unknown the program stores into an attribute its class annotates with one builtin type
(`count: int` in the class body, `self.size: int = …` where it is stored) is of that type, as a
dataclass field's is (`field_as_annotated`); literals stored stay what they are, None too where
the annotation allows it. (Before, these were unknowns of no type.)

Expectations were written from the program text before running the engine."""
import json
from typing import Optional


class Foo:
    count: int
    name: Optional[str]

    def __init__(self, s, t):
        self.count = json.loads(s)
        self.name = json.loads(t)
        self.size: int = json.loads(s)
        self.tag = json.loads(t)

    def show(self):
        a = self.count
        #@general a: types={int}
        b = self.name
        #@general b: shows={None} types={str}
        c = self.size
        #@general c: types={int}
        d = self.tag
        #@general d: untyped
        return a

    def reset(self):
        self.name = None
        n = self.name
        #@general n: exactly={None}


f = Foo("3", '"x"')
f.show()
f.reset()
