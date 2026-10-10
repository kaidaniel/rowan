"""Hypothesis: model_fields_set follows what was passed, not what has a value: exclude_unset
keeps only the passed fields; model_copy(update=) marks the updated fields as set (a copy of a
model built with `name` only dumps `price` too after a price update), does not validate the
update (a str stays a str in a float field), and is shallow (the copy shares the tags list);
Field(default_factory=list) gives each instance its own list. A record from input sets an
unknown subset of fields (only `name` surely)."""
import json
import os

from pydantic import BaseModel, Field


class Item(BaseModel):
    name: str
    price: float = 0.0
    tags: list[str] = Field(default_factory=list)
    note: str | None = None


def make(data):
    return Item(**data)                          #@ input name


def changes(item):
    return item.model_dump(exclude_unset=True)


def bump(item, pct):
    return item.model_copy(update={"price": round(item.price * (1 + pct / 100), 2)})


if __name__ == "__main__":
    i1 = make({"name": "a", "price": 2})
    ch1 = changes(i1)
    #@row ch1: present={name,price} definite={name,price} absent={tags,note} closed
    i2 = bump(i1, 50)
    p2 = i2.price                                #@value p2: exactly={3.0}
    i3 = make({"name": "b"})
    ch3 = changes(i3)
    #@row ch3: present={name} definite={name} absent={price,tags,note} closed
    ch4 = changes(bump(i3, 10))
    #@row ch4: present={name,price} definite={name,price} absent={tags,note} closed
    fs = tuple(sorted(bump(i3, 10).model_fields_set))   #@value fs: exactly={("name","price")}
    i3.tags.append("x")
    i4 = make({"name": "c"})
    n4 = len(i4.tags)                            #@value n4: exactly={0}
    cp = i3.model_copy()
    cp.tags.append("y")
    n3 = len(i3.tags)                            #@value n3: exactly={2}
    s = i1.model_copy(update={"price": "cheap"})
    sp = s.price                                 #@value sp: exactly={"cheap"}
    #@row s: present={name,price,tags,note} definite={name,price,tags,note} closed
    full = s.model_dump()
    #@row full: present={name,price,tags,note} definite={name,price,tags,note} closed
    i5 = make(json.loads(os.environ.get("ITEM", '{"name": "d", "note": "n"}')))
    #@schema json.loads@L55: always={name} only={name}
    ch5 = changes(i5)
    #@row ch5: present={name,price,tags,note} definite={name} closed
