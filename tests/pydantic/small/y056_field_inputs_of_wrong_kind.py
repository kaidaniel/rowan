"""A field annotated with a model takes a dict or an instance of it: None or another literal is a
`model_type` error, so a validated instance never holds one there. A `str` field given a str on
some paths and a dict on others holds the str (the dict is a `string_type` error).

Expectations were written from the program text before running the engine."""
import os

from pydantic import BaseModel, ValidationError


class Export(BaseModel):
    format: str = "json"


class S(BaseModel):
    name: str
    export: Export = Export()


def errs(d):
    try:
        S(**d)
        return ()
    except ValidationError as e:
        return tuple(x["type"] for x in e.errors())


a = errs({"name": "p", "export": "x"})  #@value a: exactly={("model_type",)}
c = errs({"name": "p", "export": None})  #@value c: exactly={("model_type",)}


def build(export, name):
    try:
        s = S.model_validate({"name": name, "export": export})
    except ValidationError:
        return None
    f = s.export.format
    #@value f: exactly={"csv", "json"}
    nm = s.name
    #@value nm: exactly={"p", "q"}
    return s


build({"format": "csv"}, "p")
build("x", "p")
build({}, "q")
build({"format": "csv"}, {"k": 1})

given = "p" if os.environ.get("NO_SUCH_VARIABLE") is None else {"k": 1}
try:
    one = S(name=given).name
    #@value one: exactly={"p"}
except ValidationError:
    pass
