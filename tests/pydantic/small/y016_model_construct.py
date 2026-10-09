# model_construct: no validation; defaults are filled, missing required fields are simply not set (reading one fails), extra keys dropped
from pydantic import BaseModel


class Rec(BaseModel):
    a: int
    b: str = "x"
    c: list = []


def trusted(d):
    r = Rec.model_construct(**d)
    #@row r: present={a,b,c} definite={b,c} absent={zz} closed
    return r


def read_a(r):
    return r.a                                   #@ possibly a


full = Rec.model_construct(a=1)
#@row full: present={a,b,c} definite={a,b,c} closed
bb = full.b
#@value bb: exactly={"x"}
partial = Rec.model_construct(b="y")
#@row partial: present={b,c} definite={b,c} absent={a} closed
raw = Rec.model_construct(a="not an int")
av = raw.a
#@value av: exactly={"not an int"}


def a_of_partial():
    return partial.a                             #@ missing a


if __name__ == "__main__":
    for d in ({"a": 1}, {"b": "q"}, {"a": 2, "zz": 3}):
        try:
            read_a(trusted(d))
        except AttributeError:
            pass
    try:
        a_of_partial()
    except AttributeError:
        pass
