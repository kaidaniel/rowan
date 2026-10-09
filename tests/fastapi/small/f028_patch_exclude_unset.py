# PATCH with an all-Optional model: model_dump(exclude_unset=True) has only the keys the client sent; merged into the stored record
from typing import List, Optional

from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class ItemUpdate(BaseModel):
    name: Optional[str] = None
    price: Optional[float] = None
    tags: Optional[List[str]] = None


ITEMS = {
    "a": {"name": "pen", "price": 1.0},
    "b": {"name": "ink", "price": 4.0, "tags": ["blue"]},
}


def apply_patch(stored, changes):
    #@row changes: present={name,price,tags} closed
    merged = {**stored, **changes}
    #@row merged: present={name,price,tags} definite={name,price} closed
    return merged


@app.patch("/items/{key}")
def patch_item(key: str, upd: ItemUpdate):
    if key not in ITEMS:
        raise HTTPException(status_code=404)
    changes = upd.model_dump(exclude_unset=True)
    full = upd.model_dump()
    #@row full: present={name,price,tags} definite={name,price,tags} closed
    ITEMS[key] = apply_patch(ITEMS[key], changes)
    return {"changed": sorted(changes), "item": ITEMS[key]}


if __name__ == "__main__":
    client = TestClient(app)
    assert client.patch("/items/a", json={"price": 2}).json()["changed"] == ["price"]
    assert client.patch("/items/a", json={"tags": ["x"], "name": None}).json()["changed"] == ["name", "tags"]
    assert client.patch("/items/b", json={}).json()["changed"] == []
    assert client.patch("/items/b", json={"colour": "red"}).json()["changed"] == []
    assert client.patch("/items/z", json={"price": 1}).status_code == 404
    item = client.patch("/items/b", json={"name": "INK"}).json()["item"]
    #@row item: present={name,price,tags} definite={name,price,tags} closed
