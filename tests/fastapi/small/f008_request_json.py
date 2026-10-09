# Request: await request.json() is an open record; json.loads(await request.body()) gives the schema a source line; query params via request
import json

from fastapi import FastAPI, Request
from fastapi.testclient import TestClient

app = FastAPI()

HANDLERS_SEEN = []


def route_event(kind, data):
    if kind == "click":
        ev = {"kind": kind, "x": data["x"], "y": data["y"]}  #@ input x ; input y
    else:
        ev = {"kind": kind, "text": data.get("text", "")}
    #@row ev: present={kind,x,y,text} definite={kind} closed
    return ev


@app.post("/events")
async def events(request: Request):
    data = await request.json()
    kind = data["kind"]                              #@ input kind
    #@row data: definite={kind} open
    ev = route_event(kind, data)
    HANDLERS_SEEN.append(ev)
    return ev


@app.post("/raw")
async def raw(request: Request):
    payload = json.loads(await request.body())
    user = payload["user"]                           #@ input user
    #@row payload: definite={user} open
    tags = payload.get("tags", [])
    verbose = request.query_params.get("verbose") == "1"
    out = {"user": user, "n": len(tags)}
    if verbose:
        out["keys"] = sorted(payload)
    #@row out: present={user,n,keys} definite={user,n} closed
    return out
#@schema json.loads@L33: always={user} only={user}


if __name__ == "__main__":
    client = TestClient(app)
    for body in [{"kind": "click", "x": 1, "y": 2}, {"kind": "key", "text": "a"}, {"kind": "scroll"}, {"kind": "click", "x": 1}, {"x": 1}]:
        try:
            client.post("/events", json=body)
        except KeyError:
            pass
    assert [e["kind"] for e in HANDLERS_SEEN] == ["click", "key", "scroll"]
    for body, q in [({"user": "a"}, {}), ({"user": "b", "tags": [1, 2]}, {"verbose": "1"}), ({"tags": []}, {})]:
        try:
            client.post("/raw", json=body, params=q)
        except KeyError:
            pass
