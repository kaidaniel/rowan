"""An `attrs` class's factories, `attrs.field(factory=list)` and `attrs.Factory(dict)`, make a
new object per field and per instance, as `dataclasses.field(default_factory=…)` does.

Expectations were written from the program text and attrs' documentation before running the
engine."""
import attrs


@attrs.define
class Cart:
    items: list = attrs.field(factory=list)
    saved: list = attrs.field(factory=list)
    meta: dict = attrs.Factory(dict)
    extra: dict = attrs.Factory(dict)


c = Cart()
c.items.append(1)
c.meta["k"] = 1
a = len(c.items)
#@value a: exactly={1}
b = len(c.saved)
#@value b: exactly={0}
e = len(c.extra)
#@value e: exactly={0}
d = Cart()
z = len(d.items) + len(d.meta)
#@value z: exactly={0}
