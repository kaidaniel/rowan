# Several body parameters sit under their own names in the JSON: a request whose JSON lacks one
# of them (or sends the fields flat) is a 422; one with a default (`Body(...)` with a value) may
# be left out; `Body(embed=True)` on a single parameter works the same way.
import json
import os

from fastapi import Body, FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class User(BaseModel):
    username: str


class Item(BaseModel):
    title: str


@app.put("/pair")
def put_pair(user: User, item: Item, note: str = Body("")):
    return {"u": user.username, "t": item.title, "n": note}


@app.post("/one")
def post_one(item: Item = Body(embed=True)):
    return {"t": item.title}


if __name__ == "__main__":
    client = TestClient(app)
    ok = client.put("/pair", json={"user": {"username": "a"}, "item": {"title": "b"}}).status_code
    #@value ok: exactly={200}
    flat = client.put("/pair", json={"username": "a", "title": "b"}).status_code
    #@value flat: exactly={422}
    no_item = client.put("/pair", json={"user": {"username": "c"}}).status_code
    #@value no_item: exactly={422}
    one_ok = client.post("/one", json={"item": {"title": "x"}}).status_code
    #@value one_ok: exactly={200}
    one_flat = client.post("/one", json={"title": "x"}).status_code
    #@value one_flat: exactly={422}
    sent = json.loads(os.environ.get("PAIR", '{"user": {"username": "e"}}'))
    maybe = client.put("/pair", json=sent).status_code
    #@value maybe: exactly={200,422}
