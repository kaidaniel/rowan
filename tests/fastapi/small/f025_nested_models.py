# Nested pydantic models in the body (a sub-model, a list of sub-models, an optional sub-model): model_dump gives nested dicts
from typing import List, Optional

from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class Address(BaseModel):
    street: str
    city: str
    zip: str = "00000"


class Line(BaseModel):
    sku: str
    qty: int = 1


class Order(BaseModel):
    customer: str
    ship_to: Address
    bill_to: Optional[Address] = None
    lines: List[Line]


def address_label(addr):
    _ = 0  #@row addr: present={street,city,zip} definite={street,city,zip} closed
    return f"{addr.street}, {addr.zip} {addr.city}"


def totals(order):
    counts = {}
    for ln in order.lines:
        #@row ln: present={sku,qty} definite={sku,qty} closed
        counts[ln.sku] = counts.get(ln.sku, 0) + ln.qty
    return counts


@app.post("/orders")
def place(order: Order):
    d = order.model_dump()
    #@row d: present={customer,ship_to,bill_to,lines} definite={customer,ship_to,bill_to,lines} closed
    ship = d["ship_to"]                              #@ ok ship_to
    #@row ship: present={street,city,zip} definite={street,city,zip} closed
    bill = order.bill_to or order.ship_to
    out = {"label": address_label(bill), "counts": totals(order)}
    slim = order.model_dump(exclude_none=True)
    #@row slim: present={customer,ship_to,bill_to,lines} definite={customer,ship_to,lines} closed
    out["has_bill"] = "bill_to" in slim
    return out


if __name__ == "__main__":
    client = TestClient(app)
    a = {"street": "Main 1", "city": "Oslo"}
    r = client.post("/orders", json={"customer": "ann", "ship_to": a, "lines": [{"sku": "x"}, {"sku": "x", "qty": 2}]})
    assert r.json() == {"label": "Main 1, 00000 Oslo", "counts": {"x": 3}, "has_bill": False}, r.json()
    r = client.post("/orders", json={"customer": "bob", "ship_to": a, "bill_to": {"street": "B 2", "city": "Rome", "zip": "1"}, "lines": []})
    assert r.json()["has_bill"] is True
    assert client.post("/orders", json={"customer": "c", "ship_to": {"city": "X"}, "lines": []}).status_code == 422
    assert client.post("/orders", json={"customer": "c", "ship_to": a, "lines": [{"qty": 1}]}).status_code == 422
