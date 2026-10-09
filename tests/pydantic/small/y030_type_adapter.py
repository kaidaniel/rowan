# TypeAdapter: list[Model] validation of a payload list, dict[str, int], a TypedDict (extra keys dropped), dump_python back to dicts
from typing import NotRequired, TypedDict

from pydantic import BaseModel, TypeAdapter, ValidationError


class Item(BaseModel):
    name: str
    qty: int = 1


class Opts(TypedDict):
    mode: str
    level: NotRequired[int]


ITEMS = TypeAdapter(list[Item])
OPTS = TypeAdapter(Opts)


def load_items(rows):
    items = ITEMS.validate_python(rows)          #@ possibly name
    for it in items:
        #@row it: present={name,qty} definite={name,qty} closed
        q = it.qty                               #@ ok qty
    dumped = ITEMS.dump_python(items)
    for d in dumped:
        #@row d: present={name,qty} definite={name,qty} closed
        n = d["name"]                            #@ ok name
    return items


def load_opts(raw):
    o = OPTS.validate_python(raw)
    #@row o: present={mode,level} definite={mode} closed
    return o["mode"]                             #@ ok mode


counts = TypeAdapter(dict[str, int]).validate_python({"a": "1", "b": 2})
#@row counts: present={a,b} definite={a,b} closed
ca = counts["a"]
#@value ca: exactly={1}
o1 = OPTS.validate_python({"mode": "x", "junk": 1})
#@row o1: present={mode} definite={mode} absent={junk,level} closed

if __name__ == "__main__":
    load_items([])
    load_items([{"name": "a"}, {"name": "b", "qty": 2, "x": 0}])
    try:
        load_items([{"qty": 3}])
    except ValidationError:
        pass
    load_opts({"mode": "m"})
    load_opts({"mode": "n", "level": 2, "other": 1})

#@schema param load_items.rows['*']: always={name} only={name}
#@schema param load_opts.raw: always={mode} only={mode}
