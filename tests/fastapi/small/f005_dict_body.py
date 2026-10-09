# A Dict[str, Any] body is an open record: keys read through two helper layers are what the endpoint requires of it
from typing import Any, Dict

from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient

app = FastAPI()


def normalize(payload):
    name = payload["name"].strip()                   #@ input name
    qty = int(payload.get("qty", 1))
    rec = {"name": name, "qty": qty}
    if "tags" in payload:
        rec["tags"] = sorted(payload["tags"])        #@ ok tags
    _ = 0  #@row rec: present={name,qty,tags} definite={name,qty} closed
    #@row payload: definite={name} open
    return rec


def check(rec):
    if rec["qty"] <= 0:                              #@ ok qty
        raise HTTPException(status_code=422, detail="qty must be positive")
    return rec


@app.post("/orders")
def create_order(body: Dict[str, Any]):
    rec = check(normalize(body))
    rec["customer"] = body["customer"]               #@ input customer
    #@row body: definite={name,customer} open
    #@row rec: present={name,qty,tags,customer} definite={name,qty,customer} closed
    return rec
#@schema param create_order.body: always={name,customer} only={name,customer}


if __name__ == "__main__":
    client = TestClient(app)
    bodies = [
        {"name": " pen ", "customer": "c1"},
        {"name": "ink", "qty": 3, "tags": ["z", "a"], "customer": "c2", "extra": 1},
        {"name": "cap", "qty": 0, "customer": "c3"},
        {"name": "cup", "qty": 2},
        {"qty": 2, "customer": "c4"},
    ]
    for b in bodies:
        try:
            client.post("/orders", json=b)
        except KeyError:
            pass
