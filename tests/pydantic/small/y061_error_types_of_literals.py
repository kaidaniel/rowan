# The type of a validation error depends on what was given: text that does not parse as the
# field's type is `*_parsing`, a value of another kind (None, a list) is `*_type`, a float
# with a fraction for an int `int_from_float`; a str field takes text only (`string_type`),
# a bool field text it knows, 0 / 1 and 0.0 / 1.0. Before, None for an int was `int_parsing`,
# and a str or bool field took any literal as it was.
# Expectations were written from the program text and pydantic's documented error types
# before running the engine.
from typing import Optional

from pydantic import BaseModel, ValidationError


class Item(BaseModel):
    n: int
    price: float = 0.0
    note: Optional[str] = None
    flag: bool = False


def first_error(d):
    try:
        Item(**d)
        return "ok"
    except ValidationError as e:
        return e.errors()[0]["type"]


none_int = first_error({"n": None})
#@value none_int: exactly={"int_type"}
text_int = first_error({"n": "x"})
#@value text_int: exactly={"int_parsing"}
frac_int = first_error({"n": 1.5})
#@value frac_int: exactly={"int_from_float"}
none_float = first_error({"n": 1, "price": None})
#@value none_float: exactly={"float_type"}
text_float = first_error({"n": 1, "price": "cheap"})
#@value text_float: exactly={"float_parsing"}
fine = first_error({"n": "3", "price": "2.5"})
#@value fine: exactly={"ok"}
int_note = first_error({"n": 1, "note": 5})
#@value int_note: exactly={"string_type"}
two_flag = first_error({"n": 1, "flag": 2})
#@value two_flag: exactly={"bool_parsing"}
none_flag = first_error({"n": 1, "flag": None})
#@value none_flag: exactly={"bool_type"}
yes = Item(n=1, flag="yes").flag
#@value yes: exactly={True}
one = Item(n=1, flag=1.0).flag
#@value one: exactly={True}
