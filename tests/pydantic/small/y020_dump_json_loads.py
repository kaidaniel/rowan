# model_dump_json + json.loads round trip: the same keys as model_dump (with the same include / exclude / alias options)
import json

from pydantic import BaseModel, Field


class Item(BaseModel):
    name: str
    price: float
    note: str | None = None
    sku: str = Field("?", serialization_alias="SKU")


def to_wire(item):
    data = json.loads(item.model_dump_json(exclude_none=True, by_alias=True))
    #@row data: present={name,price,note,SKU} definite={name,price,SKU} absent={sku} closed
    return data


def slim(item):
    data = json.loads(item.model_dump_json(include={"name", "price"}))
    #@row data: present={name,price} definite={name,price} closed
    return data["price"]                         #@ ok price


it = Item(name="a", price=1.0)
full = json.loads(it.model_dump_json())
#@row full: present={name,price,note,sku} definite={name,price,note,sku} closed
nt = full["note"]
#@value nt: exactly={None}
text = it.model_dump_json()
has_null = '"note":null' in text
#@value has_null: exactly={True}

if __name__ == "__main__":
    for x in (it, Item(name="b", price=2, note="n", sku="B1")):
        to_wire(x)
        slim(x)
