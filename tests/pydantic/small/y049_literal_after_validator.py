"""A `Literal` field rejects other values of its type (a ValidationError the caller catches)
and narrows unknown input to its options; an after-validator testing `self.unit == "F"` and
rewriting it leaves exactly one option."""
import json
import os
from typing import Literal

from pydantic import BaseModel, ValidationError, model_validator


class Reading(BaseModel):
    value: float
    unit: Literal["C", "F"] = "C"

    @model_validator(mode="after")
    def to_celsius(self):
        if self.unit == "F":
            self.value = round((self.value - 32) * 5 / 9, 2)
            self.unit = "C"
        return self


def parse(raw):
    try:
        return Reading.model_validate(raw).unit
    except ValidationError:
        return "bad"


if __name__ == "__main__":
    a = parse({"value": 1, "unit": "K"})         #@value a: exactly={"bad"}
    b = parse({"value": 1, "unit": "F"})         #@value b: exactly={"C"}
    v = Reading(value=212, unit="F").value       #@value v: exactly={100.0}
    raw = json.loads(os.environ.get("READING_JSON", '{"value": 50, "unit": "F"}'))
    u = parse(raw)                               #@value u: exactly={"C","bad"}
