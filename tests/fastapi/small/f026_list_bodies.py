# List bodies: List[Dict[str, Any]] (each element an open record; the schema path ['*']) and List[Model]
from typing import Any, Dict, List

from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class Reading(BaseModel):
    sensor: str
    value: float


def line_total(it):  # (its only caller read it["sku"] first: sku is definite here too; was definite={qty,price})
    total = it["qty"] * it["price"]                  #@ input qty ; input price
    #@row it: definite={sku,qty,price} open
    return total


@app.post("/bulk")
def bulk(items: List[Dict[str, Any]]):
    out = []
    for it in items:
        sku = it["sku"]                              #@ input sku
        out.append({"sku": sku, "total": line_total(it)})
    n = len(out)
    first = out[0] if out else {"sku": None, "total": 0}
    #@row first: present={sku,total} definite={sku,total} closed
    return {"n": n, "lines": out}
#@schema param bulk.items['*']: always={sku,qty,price} only={sku,qty,price}


def by_sensor(readings):
    acc = {}
    for r in readings:
        #@row r: present={sensor,value} definite={sensor,value} closed
        acc.setdefault(r.sensor, []).append(r.value)
    return {k: sum(v) / len(v) for k, v in acc.items()}


@app.post("/readings")
def readings(rs: List[Reading]):
    return by_sensor(rs)


if __name__ == "__main__":
    client = TestClient(app)
    for body in [[{"sku": "a", "qty": 2, "price": 1.5}], [{"sku": "b", "qty": 1, "price": 3, "note": "x"}, {"sku": "c", "qty": 0, "price": 9}], []]:
        try:
            client.post("/bulk", json=body)
        except KeyError:
            pass
    try:
        client.post("/bulk", json=[{"sku": "d", "qty": 1}])
    except KeyError:
        pass
    assert client.post("/readings", json=[{"sensor": "t", "value": 1}, {"sensor": "t", "value": 3}]).json() == {"t": 2.0}
    assert client.post("/readings", json=[]).json() == {}
    assert client.post("/readings", json=[{"sensor": "t"}]).status_code == 422
