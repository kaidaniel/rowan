# Several body parameters (two models and a scalar Body): each sits under its own name in the JSON; a helper merges them
from typing import Optional

from fastapi import Body, FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class User(BaseModel):
    username: str
    full_name: Optional[str] = None


class Item(BaseModel):
    title: str
    price: float = 0.0


def merge(user, item, note):
    #@row user: present={username,full_name} definite={username,full_name} closed
    #@row item: present={title,price} definite={title,price} closed
    rec = {"owner": user.username, **item.model_dump()}
    if note:
        rec["note"] = note
    if user.full_name is not None:
        rec["display"] = user.full_name
    _ = 0  #@row rec: present={owner,title,price,note,display} definite={owner,title,price} closed
    return rec


@app.put("/carts/{cart_id}")
def put_cart(cart_id: int, user: User, item: Item, note: str = Body("")):
    rec = merge(user, item, note)
    rec["cart"] = cart_id
    return rec


if __name__ == "__main__":
    client = TestClient(app)
    r = client.put("/carts/1", json={"user": {"username": "ann"}, "item": {"title": "pen"}})
    assert r.json() == {"owner": "ann", "title": "pen", "price": 0.0, "cart": 1}, r.json()
    r = client.put("/carts/2", json={"user": {"username": "bob", "full_name": "Bob B"}, "item": {"title": "ink", "price": 3}, "note": "gift"})
    assert r.json()["display"] == "Bob B" and r.json()["note"] == "gift"
    assert client.put("/carts/3", json={"username": "ann", "title": "pen"}).status_code == 422
    assert client.put("/carts/3", json={"user": {"username": "c"}}).status_code == 422
    for data in [client.put("/carts/4", json={"user": {"username": "d"}, "item": {"title": "t", "x": 1}, "y": 2}).json()]:
        #@row data: present={owner,title,price,cart} definite={owner,title,price,cart} closed
        pass
