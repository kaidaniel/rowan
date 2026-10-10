# Field constraints (min_length / max_length / gt / ge / le / pattern, also in Annotated): errors in field order
from typing import Annotated

from pydantic import BaseModel, Field, ValidationError


class Product(BaseModel):
    sku: str = Field(pattern=r"^[A-Z]{2}\d+$")
    name: Annotated[str, Field(min_length=2, max_length=10)]
    price: float = Field(gt=0)
    qty: int = Field(default=1, ge=0, le=100)
    tags: list[str] = Field(default_factory=list, max_length=2)


def problems(data):
    try:
        Product.model_validate(data)
        return []
    except ValidationError as e:
        return [(err["loc"][0], err["type"]) for err in e.errors()]


ok = problems({"sku": "AB12", "name": "pen", "price": 1.5})
n_ok = len(ok)                                                  #@value n_ok: exactly={0}
bad = problems({"sku": "ab12", "name": "x", "price": 0, "qty": 101, "tags": ["a", "b", "c"]})
nb = len(bad)                                                   #@value nb: exactly={5}
b0 = bad[0][1]                                                  #@value b0: exactly={"string_pattern_mismatch"}
b1 = bad[1][1]                                                  #@value b1: exactly={"string_too_short"}
b2 = bad[2][1]                                                  #@value b2: exactly={"greater_than"}
b3 = bad[3][1]                                                  #@value b3: exactly={"less_than_equal"}
b4 = bad[4][1]                                                  #@value b4: exactly={"too_long"}
long_name = problems({"sku": "XY1", "name": "abcdefghijk", "price": 2})[0][1]   #@value long_name: exactly={"string_too_long"}
