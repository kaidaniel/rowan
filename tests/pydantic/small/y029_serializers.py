# field_serializer changes a value, not the keys; model_serializer (plain) decides the keys; mode="wrap" can add keys to the default dump
from pydantic import BaseModel, field_serializer, model_serializer


class Money(BaseModel):
    cents: int
    currency: str = "EUR"

    @field_serializer("cents")
    def as_units(self, v):
        return v / 100


class Tag(BaseModel):
    id: int
    name: str
    internal: str = ""

    @model_serializer
    def ser(self):
        return {"id": self.id, "label": self.name.upper()}


class Stamped(BaseModel):
    id: int

    @model_serializer(mode="wrap")
    def ser(self, handler):
        d = handler(self)
        d["kind"] = type(self).__name__.lower()
        return d


def dump_all(m, t, s):
    md = m.model_dump()
    #@row md: present={cents,currency} definite={cents,currency} closed
    td = t.model_dump()
    #@row td: present={id,label} definite={id,label} absent={name,internal} closed
    sd = s.model_dump()
    #@row sd: present={id,kind} definite={id,kind} closed
    return td["label"]                           #@ ok label


mv = Money(cents=250).model_dump()["cents"]
#@value mv: exactly={2.5}
k = Stamped(id=1).model_dump()["kind"]
#@value k: exactly={"stamped"}
lab = Tag(id=1, name="ab").model_dump()["label"]
#@value lab: exactly={"AB"}

if __name__ == "__main__":
    dump_all(Money(cents=1), Tag(id=1, name="x"), Stamped(id=2))
    dump_all(Money(cents=5, currency="USD"), Tag(id=3, name="y", internal="i"), Stamped(id=4))
