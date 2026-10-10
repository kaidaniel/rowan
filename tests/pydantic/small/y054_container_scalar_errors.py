"""A scalar is no container: a str for list[str] is list_type (not its characters), for
tuple / set / frozenset tuple_type / set_type / frozen_set_type; a Sequence[str] rejects a
str as sequence_str and a number as is_instance_of; an Iterable[str] takes a str."""
from typing import Iterable, Sequence

from pydantic import BaseModel, ValidationError


class M(BaseModel):
    tags: list[str] = []
    pair: tuple[str, ...] = ()
    uniq: set[str] = set()
    frozen: frozenset[str] = frozenset()
    seq: Sequence[str] = ()
    it: Iterable[str] = ()


def err_types(data):
    try:
        M(**data)
        return ()
    except ValidationError as e:
        return tuple(x["type"] for x in e.errors())


a = err_types({"tags": "web"})                    #@value a: exactly={("list_type",)}
b = err_types({"pair": "ab", "uniq": "ab"})       #@value b: exactly={("tuple_type","set_type")}
c = err_types({"frozen": "x"})                    #@value c: exactly={("frozen_set_type",)}
d = err_types({"seq": "abc"})                     #@value d: exactly={("sequence_str",)}
e = err_types({"seq": 5})                         #@value e: exactly={("is_instance_of",)}
f = err_types({"it": "abc"})                      #@value f: exactly={()}
g = err_types({"tags": ["a", "b"]})               #@value g: exactly={()}
