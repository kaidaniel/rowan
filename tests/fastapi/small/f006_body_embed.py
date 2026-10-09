# Body(...) dict parameters, Body(embed=True) (the dict sits under the parameter's name) and two Body params (both embedded)
from typing import Any, Dict

from fastapi import Body, FastAPI
from fastapi.testclient import TestClient

app = FastAPI()


def make_note(raw):
    note = {"title": raw["title"], "body": raw.get("body", "")}
    #@row note: present={title,body} definite={title,body} closed
    return note


@app.post("/notes")
def add_note(note: dict = Body(...)):
    n = make_note(note)
    #@row note: definite={title} open
    return n
#@schema param add_note.note: always={title} only={title}


@app.post("/wrapped")
def wrapped(item: Dict[str, Any] = Body(embed=True)):
    out = {"name": item["name"], "n_keys": len(item)}  #@ input name
    #@row out: present={name,n_keys} definite={name,n_keys} closed
    return out
#@schema param wrapped.item: always={name} only={name}


@app.post("/weighted")
def weighted(item: Dict[str, Any] = Body(...), importance: int = Body(1)):
    w = {"name": item["name"], "importance": importance}
    if importance > 2:
        w["urgent"] = True
    #@row w: present={name,importance,urgent} definite={name,importance} closed
    return w
#@schema param weighted.item: always={name} only={name}


if __name__ == "__main__":
    client = TestClient(app)
    for b in [{"title": "a"}, {"title": "b", "body": "text", "x": 1}, {"body": "no title"}]:
        try:
            client.post("/notes", json=b)
        except KeyError:
            pass
    r = client.post("/wrapped", json={"item": {"name": "pen", "colour": "red"}})
    assert r.json() == {"name": "pen", "n_keys": 2}
    assert client.post("/wrapped", json={"name": "pen"}).status_code == 422
    try:
        client.post("/wrapped", json={"item": {"label": "x"}})
    except KeyError:
        pass
    assert client.post("/weighted", json={"item": {"name": "a"}}).json() == {"name": "a", "importance": 1}
    assert client.post("/weighted", json={"item": {"name": "b", "z": 0}, "importance": 5}).json()["urgent"]
    assert client.post("/weighted", json={"name": "c"}).status_code == 422
