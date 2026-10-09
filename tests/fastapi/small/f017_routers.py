# APIRouter + include_router(prefix=): a nested router prefix, handlers in routers share a helper and an in-memory store
from typing import Any, Dict

from fastapi import APIRouter, FastAPI, HTTPException
from fastapi.testclient import TestClient

STORE = {"items": {}, "users": {}}


def save(kind, key, rec):
    bucket = STORE[kind]                             #@ ok items ; ok users
    stored = {"key": key, **rec}
    bucket[key] = stored
    # Corrected on re-reading: every caller passes a dict it built itself (closed), so `stored`
    # is the join of those: closed.
    #@row stored: present={key,title,qty,email,admin} definite={key} closed
    return stored


items = APIRouter(prefix="/items", tags=["items"])
users = APIRouter()


@items.post("/{key}")
def put_item(key: str, body: Dict[str, Any]):
    rec = {"title": body["title"], "qty": body.get("qty", 0)}  #@ input title
    out = save("items", key, rec)
    #@row out: present={key,title,qty} definite={key,title,qty} closed
    return out
#@schema param put_item.body: always={title} only={title}


@items.get("/{key}")
def get_item(key: str):
    if key not in STORE["items"]:
        raise HTTPException(status_code=404)
    it = STORE["items"][key]
    t = it["title"]                                  #@ ok title
    return it


@users.post("/users/{name}")
def put_user(name: str, body: Dict[str, Any]):
    rec = {"email": body["email"]}                   #@ input email
    if body.get("admin"):
        rec["admin"] = True
    out = save("users", name, rec)
    #@row out: present={key,email,admin} definite={key,email} closed
    return out
#@schema param put_user.body: always={email} only={email}


app = FastAPI()
app.include_router(items)
app.include_router(users, prefix="/v1")


if __name__ == "__main__":
    client = TestClient(app)
    assert client.post("/items/a", json={"title": "pen"}).json() == {"key": "a", "title": "pen", "qty": 0}
    assert client.post("/items/b", json={"title": "ink", "qty": 2, "x": 1}).status_code == 200
    try:
        client.post("/items/c", json={"qty": 1})
    except KeyError:
        pass
    assert client.get("/items/a").json()["title"] == "pen"
    assert client.get("/items/zz").status_code == 404
    assert client.post("/v1/users/ann", json={"email": "a@x", "admin": True}).json().get("admin") in (True, None)
    assert client.post("/v1/users/bob", json={"email": "b@x", "nick": "b"}).status_code == 200
    assert client.post("/users/bob", json={"email": "b@x"}).status_code == 404
    try:
        client.post("/v1/users/cyd", json={})
    except KeyError:
        pass
    _ = 0  #@row STORE: present={items,users} definite={items,users} closed
