"""pydantic v2 validates another class's object (an instance, a SimpleNamespace, another
model) as a model only `from_attributes`: the model's config `from_attributes=True` (for
that model alone: a nested model needs its own), or `model_validate(…, from_attributes=True)`
(for nested models too). Otherwise the error is `model_type`. Rowan read such an object's
attributes as the fields.

Expectations were written from the program text and pydantic's documented behaviour before
running the engine."""
from types import SimpleNamespace

from pydantic import BaseModel, ConfigDict, ValidationError


class Addr(BaseModel):
    city: str


class Plain(BaseModel):
    addr: Addr


class FromAttrs(BaseModel):
    model_config = ConfigDict(from_attributes=True)
    city: str


class Outer(BaseModel):
    model_config = ConfigDict(from_attributes=True)
    addr: Addr


class Other(BaseModel):
    city: str


class Obj:
    def __init__(self):
        self.city = "x"


def kind(f):
    try:
        f()
        return "ok"
    except ValidationError as e:
        return e.errors()[0]["type"]


k1 = kind(lambda: Addr.model_validate(Obj()))
#@value k1: exactly={"model_type"}
k2 = kind(lambda: Addr.model_validate(Obj(), from_attributes=True))
#@value k2: exactly={"ok"}
k3 = kind(lambda: Plain(addr=Obj()))
#@value k3: exactly={"model_type"}
k4 = kind(lambda: Plain(addr=Other(city="x")))
#@value k4: exactly={"model_type"}
k5 = kind(lambda: Plain(addr=SimpleNamespace(city="x")))
#@value k5: exactly={"model_type"}
k6 = kind(lambda: FromAttrs.model_validate(Obj()))
#@value k6: exactly={"ok"}
k7 = kind(lambda: Outer(addr=Obj()))
#@value k7: exactly={"model_type"}
k8 = kind(lambda: Plain.model_validate({"addr": Obj()}, from_attributes=True))
#@value k8: exactly={"ok"}
k9 = kind(lambda: Plain(addr={"city": "x"}))
#@value k9: exactly={"ok"}
c = FromAttrs.model_validate(Obj()).city
#@value c: exactly={"x"}
