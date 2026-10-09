# Checkout service: nested pydantic order models, an inventory repository, pricing helpers with raw coupon dicts, background receipts, custom exceptions with handlers, response_model_exclude_none
from typing import Any, Dict, List, Optional

from fastapi import BackgroundTasks, Body, FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse
from fastapi.testclient import TestClient
from pydantic import BaseModel, Field


class Address(BaseModel):
    street: str
    city: str
    country: str = "NO"


class LineIn(BaseModel):
    sku: str
    qty: int = Field(1, ge=1)


class OrderIn(BaseModel):
    customer: str
    lines: List[LineIn]
    ship_to: Address
    coupon: Optional[str] = None
    gift_note: Optional[str] = None


class OrderOut(BaseModel):
    id: int
    customer: str
    total: float
    discount: Optional[float] = None
    gift_note: Optional[str] = None
    lines: List[Dict[str, Any]]


# ---------------------------------------------------------------- inventory

CATALOG = {
    "pen": {"price": 2.5, "stock": 10},
    "ink": {"price": 7.0, "stock": 1},
    "pad": {"price": 4.0, "stock": 0, "backorder": True},
}

COUPONS = {"TENOFF": {"kind": "percent", "value": 10}, "FIVE": {"kind": "fixed", "value": 5.0, "min_total": 20.0}}


class OutOfStock(Exception):
    def __init__(self, sku, wanted, have):
        self.sku = sku
        self.wanted = wanted
        self.have = have


class UnknownSku(Exception):
    def __init__(self, sku):
        self.sku = sku


def reserve(sku, qty):
    item = CATALOG.get(sku)
    if item is None:
        raise UnknownSku(sku)
    _ = 0  #@row item: present={price,stock,backorder} definite={price,stock} closed
    if item["stock"] < qty and not item.get("backorder"):  #@ ok stock
        raise OutOfStock(sku, qty, item["stock"])
    item["stock"] = max(0, item["stock"] - qty)
    return item["price"]                             #@ ok price


# ---------------------------------------------------------------- pricing

def price_lines(lines):
    priced = []
    for ln in lines:
        #@row ln: present={sku,qty} definite={sku,qty} closed
        unit = reserve(ln.sku, ln.qty)
        row = {"sku": ln.sku, "qty": ln.qty, "unit": unit, "amount": round(unit * ln.qty, 2)}
        #@row row: present={sku,qty,unit,amount} definite={sku,qty,unit,amount} closed
        priced.append(row)
    return priced


def apply_coupon(code, subtotal):
    if code is None:
        return 0.0
    c = COUPONS.get(code)
    if c is None:
        raise HTTPException(status_code=400, detail={"error": "bad coupon", "code": code})
    _ = 0  #@row c: present={kind,value,min_total} definite={kind,value} closed
    if subtotal < c.get("min_total", 0):
        return 0.0
    if c["kind"] == "percent":                       #@ ok kind
        return round(subtotal * c["value"] / 100, 2)  #@ ok value
    return min(c["value"], subtotal)


def custom_discount(rule, subtotal):
    pct = rule["percent"]                            #@ input percent
    cap = rule.get("cap", subtotal)
    #@row rule: definite={percent} open
    return min(round(subtotal * pct / 100, 2), cap)


# ---------------------------------------------------------------- receipts (background)

RECEIPTS = []
ORDERS = {}


def send_receipt(order, address):
    _ = 0  #@row order: present={id,customer,total,discount,gift_note,lines} definite={id,customer,total,discount,gift_note,lines} closed
    #@row address: present={street,city,country} definite={street,city,country} closed
    rec = {"to": order["customer"], "total": order["total"], "city": address["city"]}  #@ ok customer ; ok total ; ok city
    if order["gift_note"]:                           #@ ok gift_note
        rec["note"] = order["gift_note"]
    _ = 0  #@row rec: present={to,total,city,note} definite={to,total,city} closed
    RECEIPTS.append(rec)


# ---------------------------------------------------------------- app

app = FastAPI()


@app.exception_handler(OutOfStock)
async def out_of_stock(request: Request, exc: OutOfStock):
    _ = 0  #@row exc: present={sku,wanted,have} definite={sku,wanted,have} closed
    return JSONResponse(status_code=409, content={"error": "out_of_stock", "sku": exc.sku, "have": exc.have})


@app.exception_handler(UnknownSku)
async def unknown_sku(request: Request, exc: UnknownSku):
    return JSONResponse(status_code=404, content={"error": "unknown_sku", "sku": exc.sku})


def build_order(order, lines, discount):
    subtotal = sum(l["amount"] for l in lines)
    oid = len(ORDERS) + 1
    out = {"id": oid, "customer": order.customer, "total": round(subtotal - discount, 2),
           "discount": discount or None, "gift_note": order.gift_note, "lines": lines}
    _ = 0  #@row out: present={id,customer,total,discount,gift_note,lines} definite={id,customer,total,discount,gift_note,lines} closed
    ORDERS[oid] = out
    return out


@app.post("/orders", response_model=OrderOut, response_model_exclude_none=True, status_code=201)
def place_order(order: OrderIn, tasks: BackgroundTasks):
    lines = price_lines(order.lines)
    subtotal = sum(l["amount"] for l in lines)
    discount = apply_coupon(order.coupon, subtotal)
    out = build_order(order, lines, discount)
    tasks.add_task(send_receipt, out, order.ship_to.model_dump())
    return out


@app.post("/quotes")
def quote(lines: List[Dict[str, Any]] = Body(...), rule: Optional[Dict[str, Any]] = Body(None)):
    subtotal = 0.0
    for ln in lines:
        item = CATALOG[ln["sku"]]                    #@ input sku
        subtotal += item["price"] * ln.get("qty", 1)  #@ ok price
    disc = custom_discount(rule, subtotal) if rule else 0.0
    q = {"subtotal": round(subtotal, 2), "discount": disc, "total": round(subtotal - disc, 2)}
    #@row q: present={subtotal,discount,total} definite={subtotal,discount,total} closed
    return q
#@schema param quote.lines['*']: always={sku} only={sku}
#@schema param quote.rule: always={percent} only={percent}


@app.get("/orders/{oid}", response_model=OrderOut)
def get_order(oid: int):
    if oid not in ORDERS:
        raise HTTPException(status_code=404)
    o = ORDERS[oid]
    #@row o: present={id,customer,total,discount,gift_note,lines} definite={id,customer,total,discount,gift_note,lines} closed
    return o


if __name__ == "__main__":
    client = TestClient(app)
    addr = {"street": "Main 1", "city": "Oslo"}
    r = client.post("/orders", json={"customer": "ann", "lines": [{"sku": "pen", "qty": 2}, {"sku": "ink"}], "ship_to": addr})
    assert r.status_code == 201, r.text
    o1 = r.json()
    #@row o1: present={id,customer,total,discount,gift_note,lines} definite={id,customer,total,lines} closed
    assert "discount" not in o1 and o1["total"] == 12.0
    r = client.post("/orders", json={"customer": "bob", "lines": [{"sku": "pen", "qty": 8}], "ship_to": {**addr, "country": "SE"},
                                      "coupon": "TENOFF", "gift_note": "enjoy"})
    o2 = r.json()
    #@row o2: present={id,customer,total,discount,gift_note,lines} definite={id,customer,total,discount,gift_note,lines} closed
    r = client.post("/orders", json={"customer": "cyd", "lines": [{"sku": "ink"}], "ship_to": addr})
    assert r.status_code == 409 and r.json()["sku"] == "ink"
    r = client.post("/orders", json={"customer": "cyd", "lines": [{"sku": "zzz"}], "ship_to": addr})
    assert r.status_code == 404 and r.json() == {"error": "unknown_sku", "sku": "zzz"}
    r = client.post("/orders", json={"customer": "dan", "lines": [{"sku": "pad", "qty": 3}], "ship_to": addr, "coupon": "FIVE"})
    assert r.status_code == 201 and "discount" not in r.json()
    assert client.post("/orders", json={"customer": "eve", "lines": [{"sku": "pad"}], "ship_to": addr, "coupon": "NOPE"}).status_code == 400
    assert client.post("/orders", json={"customer": "eve", "lines": [{"sku": "pen", "qty": 0}], "ship_to": addr}).status_code == 422
    assert client.post("/orders", json={"customer": "eve", "lines": [], "ship_to": {"street": "x"}}).status_code == 422

    assert [x["to"] for x in RECEIPTS] == ["ann", "bob", "dan"]
    gift = RECEIPTS[1]
    #@row gift: present={to,total,city,note} definite={to,total,city,note} closed

    q1 = client.post("/quotes", json={"lines": [{"sku": "pen", "qty": 2}, {"sku": "pad"}]}).json()
    assert q1 == {"subtotal": 9.0, "discount": 0.0, "total": 9.0}, q1
    q2 = client.post("/quotes", json={"lines": [{"sku": "pen", "note": "x"}], "rule": {"percent": 50, "cap": 1.0}}).json()
    assert q2["discount"] == 1.0
    client.post("/quotes", json={"lines": [{"sku": "pad", "qty": 1}], "rule": {"percent": 25, "label": "spring"}})
    for bad in [{"lines": [{"sku": "nope"}]}, {"lines": [{"qty": 1}]}, {"lines": [{"sku": "pen"}], "rule": {"pct": 5}}]:
        try:
            client.post("/quotes", json=bad)
        except KeyError:
            pass

    g = client.get("/orders/1").json()
    #@row g: present={id,customer,total,discount,gift_note,lines} definite={id,customer,total,discount,gift_note,lines} closed
    assert g["discount"] is None
    assert client.get("/orders/42").status_code == 404
