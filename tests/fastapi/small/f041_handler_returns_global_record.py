# A route handler that returns a record of a global gives it to the framework, which only
# serializes it: for any request the records keep their values; a handler that takes one out
# and changes it changes it.
from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()

ITEMS = {1: {"id": 1, "name": "pen"}, 2: {"id": 2, "name": "ink"}}
PEN = ITEMS[1]
LOG = [{"seen": 0}]
FIRST = LOG[0]


@app.get("/items/{iid}")
def get_item(iid: int):
    return ITEMS.get(iid, {})


@app.post("/items/{iid}")
def put_item(iid: int, name: str):
    ITEMS[iid] = {"id": iid, "name": name}
    return {"ok": True}


@app.get("/log")
def last_log():
    entry = LOG[-1]
    entry["seen"] += 1
    return entry


def pen_name():
    n = PEN["name"]
    # (get_item hands records to the framework; put_item replaces ITEMS' items, not them)
    #@value n: exactly={"pen"}
    #@general n: exactly={"pen"}
    return n


def first_seen():
    s = FIRST["seen"]
    # (last_log changes the entry it takes out of LOG)
    #@value s: exactly={1}
    #@general s: unknown
    return s


if __name__ == "__main__":
    client = TestClient(app)
    assert client.get("/items/1").json() == {"id": 1, "name": "pen"}
    assert client.post("/items/3", params={"name": "cup"}).json() == {"ok": True}
    assert client.get("/log").json() == {"seen": 1}
    assert pen_name() == "pen"
    assert first_seen() == 1
