# A pydantic model as the request body: fields with defaults, Optional; the instance flows into helpers; model_dump keys
from typing import List, Optional

from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class Item(BaseModel):
    name: str
    price: float
    tags: List[str] = []
    note: Optional[str] = None


def price_with_tax(item, rate):
    return round(item.price * (1 + rate), 2)


def summarize(item):
    _ = 0  #@row item: present={name,price,tags,note} definite={name,price,tags,note} closed
    d = item.model_dump()
    #@row d: present={name,price,tags,note} definite={name,price,tags,note} closed
    d["n_tags"] = len(item.tags)
    slim = item.model_dump(exclude_none=True, exclude={"tags"})
    #@row slim: present={name,price,note} definite={name,price} absent={tags} closed
    return d


def sku_of(item):
    return item.sku                                  #@ missing sku


@app.post("/items")
def create_item(item: Item):
    s = summarize(item)
    s["gross"] = price_with_tax(item, 0.25)
    try:
        s["sku"] = sku_of(item)
    except AttributeError:
        pass
    _ = 0  #@row s: present={name,price,tags,note,n_tags,gross} definite={name,price,tags,note,n_tags,gross} closed
    return s


if __name__ == "__main__":
    client = TestClient(app)
    for body in [
        {"name": "pen", "price": 2.0},
        {"name": "ink", "price": 4.0, "tags": ["b", "a"], "note": "blue", "colour": "x"},
        {"name": "cap", "price": 1, "note": None},
    ]:
        data = client.post("/items", json=body).json()
        #@row data: present={name,price,tags,note,n_tags,gross} definite={name,price,tags,note,n_tags,gross} absent={colour,sku} closed
    assert client.post("/items", json={"name": "x"}).status_code == 422
    assert client.post("/items", json={"price": 3}).status_code == 422
