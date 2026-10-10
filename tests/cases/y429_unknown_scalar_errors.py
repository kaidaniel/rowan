"""Hypothesis: an unknown input validated as a float may fail as float_parsing (a string that is
no number) or float_type; as an int also int_parsing / int_from_float; a value already of the
type (an int validated before) converts. Exact for the literal records."""
import json
import os

from pydantic import BaseModel, ValidationError


class M(BaseModel):
    amount: float
    n: int = 0


def err_type(rec):
    try:
        M.model_validate(rec)
        return None
    except ValidationError as e:
        return e.errors()[0]["type"]


a = err_type({"amount": "abc"})                  #@value a: exactly={"float_parsing"}
b = err_type({"amount": [1]})                    #@value b: exactly={"float_type"}
c = err_type({"amount": 1, "n": 1.5})            #@value c: exactly={"int_from_float"}
d = err_type({"amount": "2"})                    #@value d: exactly={None}
rec = json.loads(os.environ.get("Y429_R", '{"amount": "x"}'))
e = err_type(rec)                                #@value e: includes={"float_parsing","float_type","missing","int_parsing","int_from_float",None}
m = M(amount=1, n=2)
f = err_type({"amount": m.amount, "n": m.n})     #@value f: exactly={None}
