"""Hypothesis: a list of nested models dumps as a list of dicts with each line's fields; a nested
exclude spec {"lines": {"__all__": {"meta"}}} drops meta from every line but keeps the order's
own keys and its computed field; include={"id", "count"} keeps a computed field too; the JSON
round trip has every line field; validating a list that already holds a Line instance keeps
that very instance; default_factory gives each line its own meta dict. Input orders require
`id`, and `sku` of each line."""
import json
import os

from pydantic import BaseModel, Field, computed_field


class Line(BaseModel):
    sku: str
    qty: int = 1
    meta: dict[str, str] = Field(default_factory=dict)


class Order(BaseModel):
    id: int
    lines: list[Line] = []

    @computed_field
    @property
    def count(self) -> int:
        return sum(ln.qty for ln in self.lines)


def summarize(data):
    o = Order.model_validate(data)
    return o.model_dump(exclude={"lines": {"__all__": {"meta"}}})


if __name__ == "__main__":
    d = summarize({"id": 1, "lines": [{"sku": "a", "qty": 2}, {"sku": "b"}]})
    #@row d: present={id,lines,count} definite={id,lines,count} closed
    l0 = d["lines"][0]
    #@row l0: present={sku,qty} definite={sku,qty} absent={meta} closed
    q1 = d["lines"][1]["qty"]                    #@value q1: exactly={1}
    cnt = d["count"]                             #@value cnt: exactly={3}
    o = Order(id=2, lines=[Line(sku="x"), Line(sku="y", qty=5)])
    small = o.model_dump(include={"id", "count"})
    #@row small: present={id,count} definite={id,count} absent={lines} closed
    sc = small["count"]                          #@value sc: exactly={6}
    back = json.loads(o.model_dump_json())
    #@row back: present={id,lines,count} definite={id,lines,count} closed
    b1 = back["lines"][1]
    #@row b1: present={sku,qty,meta} definite={sku,qty,meta} closed
    o.lines[0].meta["k"] = "v"
    nm = len(o.lines[1].meta)                    #@value nm: exactly={0}
    ln = Line(sku="z", qty=4)
    o2 = Order.model_validate({"id": 3, "lines": [ln]})
    same = o2.lines[0] is ln                     #@value same: exactly={True}
    d3 = summarize(json.loads(os.environ.get("ORDER", '{"id": 4, "lines": [{"sku": "q", "x": 1}]}')))
    #@schema json.loads@L54: always={id} only={id}
    #@schema json.loads@L54['lines']['*']: always={sku} only={sku}
    _ = 0                                        #@row d3: present={id,lines,count} definite={id,lines,count} closed
