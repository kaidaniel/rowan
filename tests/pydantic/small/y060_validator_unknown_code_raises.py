"""A validator that calls library code rowan does not model (`ipaddress.ip_address`) may get a
ValueError from it: pydantic turns that into an error of its field, beside the other fields'
errors. (Before, an exception of code rowan cannot see was taken to escape validation only,
and the errors listed only the other field's.)

Expectations were written from the program text before running the engine."""
import ipaddress
from typing import Literal

from pydantic import BaseModel, ValidationError, field_validator


class Host(BaseModel):
    addr: str
    kind: Literal["web", "db"] = "web"

    @field_validator("addr")
    @classmethod
    def check(cls, v):
        ipaddress.ip_address(v)
        return v


def problems(raw):
    try:
        Host.model_validate(raw)
        return ()
    except ValidationError as e:
        return tuple(sorted(err["loc"][0] for err in e.errors()))


both = problems({"addr": "not an address", "kind": "mail"})
has_addr = "addr" in both
#@value has_addr: includes={True}
