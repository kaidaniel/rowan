"""Hypothesis: a nested model's validation error does not stop the outer validation: every
field's error is in errors(), the nested one under its field's path (loc ("e", "format"));
a model validator of the nested model reports at ("e",)."""
from typing import Literal

from pydantic import BaseModel, Field, ValidationError, model_validator


class E(BaseModel):
    format: Literal["json", "csv"] = "json"
    level: int = 0

    @model_validator(mode="after")
    def chk(self):
        if self.level > 3:
            raise ValueError("too deep")
        return self


class S(BaseModel):
    n: int = Field(1, gt=0)
    e: E = E()
    name: str


def errs(data):
    try:
        S.model_validate(data)
        return []
    except ValidationError as ex:
        return [(".".join(str(p) for p in err["loc"]), err["type"]) for err in ex.errors()]


a = errs({"n": 0, "e": {"format": "xml"}, "name": "x"})
ta = tuple(a)                                    #@value ta: exactly={(("n","greater_than"),("e.format","literal_error"))}
na = len(a)                                      #@value na: exactly={2}
b = errs({"e": {"level": 5}})
b0 = b[0]                                        #@value b0: exactly={("e","value_error")}
b1 = b[1]                                        #@value b1: exactly={("name","missing")}
c = dict(errs({"n": 0}))
#@row c: present={n,name} definite={n,name} closed
