# Inventory service: CSV stock imports (csv.DictReader on a text field), webhooks dispatched by "type" through a handler table, nested routers with prefixes and a router-level API-key dependency, defaultdict stock moves, response_model filtering
import csv
import io
from collections import defaultdict
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, Body, Depends, FastAPI, Header, HTTPException
from fastapi.testclient import TestClient
from pydantic import BaseModel

STOCK: Dict[str, Dict[str, Any]] = {
    "pen": {"sku": "pen", "qty": 10, "location": "A1", "cost": 0.5},
    "ink": {"sku": "ink", "qty": 0, "location": "B2", "cost": 2.0},
}
MOVES = defaultdict(list)
WEBHOOK_LOG: List[Dict[str, Any]] = []


class StockOut(BaseModel):
    sku: str
    qty: int
    location: Optional[str] = None


# ---------------------------------------------------------------- auth

def api_key(x_api_key: str = Header(...)):
    if x_api_key not in ("inv-1", "inv-2"):
        raise HTTPException(status_code=401, detail="bad key")
    return x_api_key


# ---------------------------------------------------------------- stock helpers

def move(sku, delta, reason):
    if sku not in STOCK:
        STOCK[sku] = {"sku": sku, "qty": 0, "location": None, "cost": 0.0}
    row = STOCK[sku]
    #@row row: present={sku,qty,location,cost} definite={sku,qty,location,cost} closed
    if row["qty"] + delta < 0:                       #@ ok qty
        raise HTTPException(status_code=409, detail={"sku": sku, "have": row["qty"], "delta": delta})
    row["qty"] += delta
    rec = {"delta": delta, "reason": reason}
    MOVES[sku].append(rec)
    return row


def parse_stock_csv(text):
    rows = []
    for rec in csv.DictReader(io.StringIO(text)):
        #@row rec: open
        sku = rec["sku"].strip()                     #@ input sku
        qty = int(rec["qty"])                        #@ input qty
        #@row rec: definite={sku,qty} open
        loc = rec.get("location") or None
        rows.append({"sku": sku, "qty": qty, "location": loc})
    return rows


def parse_fixed_csv(text):
    reader = csv.DictReader(io.StringIO(text), fieldnames=["sku", "qty"])
    out = []
    for rec in reader:
        #@row rec: present={sku,qty,None} definite={sku,qty} closed
        out.append((rec["sku"], int(rec["qty"] or 0)))  #@ ok sku ; ok qty
    return out


# ---------------------------------------------------------------- webhook handlers

def on_shipment(data):
    lines = data["lines"]                            #@ input lines
    for ln in lines:
        move(ln["sku"], int(ln["qty"]), "shipment")  #@ input sku ; input qty
    return {"handled": "shipment", "n": len(lines)}


def on_sale(data):
    row = move(data["sku"], -int(data.get("qty", 1)), "sale")  #@ input sku
    out = {"handled": "sale", "left": row["qty"]}    #@ ok qty
    if row["qty"] == 0:
        out["reorder"] = True
    _ = 0  #@row out: present={handled,left,reorder} definite={handled,left} closed
    return out


def on_relocate(data):
    sku = data["sku"]                                #@ input sku
    if sku not in STOCK:
        raise HTTPException(status_code=404, detail={"sku": sku})
    STOCK[sku]["location"] = data["to"]              #@ input to
    return {"handled": "relocate", "location": data["to"]}  #@ ok to


HANDLERS = {"shipment": on_shipment, "sale": on_sale, "relocate": on_relocate}
#@row HANDLERS: present={shipment,sale,relocate} definite={shipment,sale,relocate} closed


# ---------------------------------------------------------------- routes

stock = APIRouter(prefix="/stock")
hooks = APIRouter(prefix="/hooks")
internal = APIRouter(dependencies=[Depends(api_key)])


@stock.get("/{sku}", response_model=StockOut)
def get_stock(sku: str):
    if sku not in STOCK:
        raise HTTPException(status_code=404)
    row = STOCK[sku]
    cost = row["cost"]                               #@ ok cost
    return row


@stock.post("/import")
def import_csv(body: Dict[str, Any] = Body(...)):
    rows = parse_stock_csv(body["csv"])              #@ input csv
    mode = body.get("mode", "add")
    for r in rows:
        delta = r["qty"] if mode == "add" else r["qty"] - STOCK.get(r["sku"], {"qty": 0})["qty"]  #@ ok qty
        row = move(r["sku"], delta, "import")        #@ ok sku
        if r["location"]:                            #@ ok location
            row["location"] = r["location"]
    return {"imported": len(rows), "mode": mode}
#@schema param import_csv.body: always={csv} only={csv}


@stock.post("/import-plain")
def import_plain(text: str = Body(..., embed=True)):
    pairs = parse_fixed_csv(text)
    for sku, qty in pairs:
        move(sku, qty, "plain")
    return {"imported": len(pairs)}


@hooks.post("/{source}")
def webhook(source: str, event: Dict[str, Any]):
    kind = event["type"]                             #@ input type
    handler = HANDLERS.get(kind)
    if handler is None:
        raise HTTPException(status_code=400, detail={"error": "unknown type", "type": kind})
    result = handler(event["data"])                  #@ input data
    entry = {"source": source, "type": kind, **result}
    #@row entry: present={source,type,handled,n,left,reorder,location} definite={source,type,handled} closed
    WEBHOOK_LOG.append(entry)
    return entry
#@schema param webhook.event: always={type,data} only={type,data}


@internal.get("/moves/{sku}")
def moves(sku: str):
    hist = MOVES.get(sku, [])
    total = sum(m["delta"] for m in hist)            #@ ok delta
    return {"sku": sku, "n": len(hist), "total": total}


internal.include_router(stock)
internal.include_router(hooks)
app = FastAPI()
app.include_router(internal, prefix="/v1")


if __name__ == "__main__":
    client = TestClient(app)
    k = {"X-API-Key": "inv-1"}
    assert client.get("/v1/stock/pen").status_code == 422
    assert client.get("/v1/stock/pen", headers={"X-API-Key": "nope"}).status_code == 401
    pen = client.get("/v1/stock/pen", headers=k).json()
    #@row pen: present={sku,qty,location} definite={sku,qty,location} absent={cost} closed

    r = client.post("/v1/stock/import", json={"csv": "sku,qty,location\npen,5,A9\npad,3,\n"}, headers=k)
    assert r.json() == {"imported": 2, "mode": "add"}, r.json()
    r = client.post("/v1/stock/import", json={"csv": "sku,qty\nink,4\n", "mode": "set", "who": "ops"}, headers=k)
    assert STOCK["ink"]["qty"] == 4
    for bad in [{"text": "sku,qty\n"}, {"csv": "id,qty\nx,1\n"}, {"csv": "sku,count\nx,1\n"}]:
        try:
            client.post("/v1/stock/import", json=bad, headers=k)
        except KeyError:
            pass
    assert client.post("/v1/stock/import-plain", json={"text": "cap,2\ncap,1\nlid\n"}, headers=k).json() == {"imported": 3}
    pad = client.get("/v1/stock/pad", headers=k).json()
    #@row pad: present={sku,qty,location} definite={sku,qty,location} closed
    assert pad["location"] is None

    e1 = client.post("/v1/hooks/erp", json={"type": "shipment", "data": {"lines": [{"sku": "ink", "qty": 2}, {"sku": "new", "qty": "1"}]}}, headers=k).json()
    #@row e1: present={source,type,handled,n} definite={source,type,handled,n} closed
    e2 = client.post("/v1/hooks/shop", json={"type": "sale", "data": {"sku": "new"}, "id": 7}, headers=k).json()
    #@row e2: present={source,type,handled,left,reorder} definite={source,type,handled,left,reorder} closed
    assert client.post("/v1/hooks/shop", json={"type": "sale", "data": {"sku": "new", "qty": 5}}, headers=k).status_code == 409
    e3 = client.post("/v1/hooks/wms", json={"type": "relocate", "data": {"sku": "pen", "to": "C3"}}, headers=k).json()
    #@row e3: present={source,type,handled,location} definite={source,type,handled,location} closed
    assert client.post("/v1/hooks/wms", json={"type": "relocate", "data": {"sku": "zzz", "to": "C3"}}, headers=k).status_code == 404
    assert client.post("/v1/hooks/x", json={"type": "refund", "data": {}}, headers=k).status_code == 400
    for bad in [{"data": {}}, {"type": "sale"}, {"type": "shipment", "data": {"items": []}}, {"type": "relocate", "data": {"sku": "pen"}}]:
        try:
            client.post("/v1/hooks/x", json=bad, headers=k)
        except KeyError:
            pass

    m = client.get("/v1/moves/new", headers=k).json()
    assert m == {"sku": "new", "n": 2, "total": 0}, m
    assert client.get("/v1/moves/none", headers=k).json()["n"] == 0
    kinds = sorted({e["type"] for e in WEBHOOK_LOG})
    assert kinds == ["relocate", "sale", "shipment"], kinds
    last_move = MOVES["new"][-1]
    #@row last_move: present={delta,reason} definite={delta,reason} closed
