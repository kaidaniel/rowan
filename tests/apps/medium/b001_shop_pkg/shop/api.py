"""The HTTP side: orders, shipping, cancellation, stock and stats."""
from fastapi import FastAPI, HTTPException

from shop.events import EventLog, Kind
from shop.models import OrderIn, OrderOut
from shop.pricing import quote
from shop.repo import Inventory, OrderRepo

app = FastAPI()
INVENTORY = Inventory({"A1": 5, "B2": 1, "C3": 10})
ORDERS = OrderRepo()
LOG = EventLog()


@app.post("/orders", status_code=201)
def create_order(order: OrderIn):
    short = INVENTORY.reserve(order.items)
    if short:
        LOG.emit(Kind.REJECTED, "-", short=short, customer=order.customer)
        raise HTTPException(status_code=409, detail={"short": short})
    q = quote(order, order.country())
    rec = ORDERS.add(order, q)
    LOG.emit(Kind.PLACED, rec["order_id"], total=q["total"])
    return OrderOut(**rec).model_dump(exclude_none=True)


@app.get("/orders/{oid}")
def get_order(oid: str):
    rec = ORDERS.get(oid)
    if rec is None:
        raise HTTPException(status_code=404, detail=f"no order {oid}")
    return rec


@app.post("/orders/{oid}/ship")
def ship_order(oid: str, body: dict):
    if ORDERS.get(oid) is None:
        raise HTTPException(status_code=404, detail=f"no order {oid}")
    carrier = body["carrier"]
    tracking = f"{carrier.upper()}-{oid}"
    rec = ORDERS.set_status(oid, "shipped", tracking=tracking)
    LOG.emit(Kind.SHIPPED, oid, carrier=carrier)
    return OrderOut(**rec).model_dump(exclude_none=True)


@app.post("/orders/{oid}/cancel")
def cancel_order(oid: str):
    rec = ORDERS.get(oid)
    if rec is None:
        raise HTTPException(status_code=404, detail=f"no order {oid}")
    if rec["status"] == "shipped":
        raise HTTPException(status_code=409, detail="already shipped")
    INVENTORY.release(ORDERS.lines[oid])
    ORDERS.set_status(oid, "cancelled")
    LOG.emit(Kind.CANCELLED, oid)
    return {"order_id": oid, "status": "cancelled"}


@app.post("/stock")
def restock(payload: dict):
    sku = payload["sku"]
    qty = int(payload["qty"])
    res = {"sku": sku, "stock": INVENTORY.restock(sku, qty)}
    if payload.get("reason") == "return":
        res["ref"] = payload["ref"]
    return res


@app.get("/stock/{sku}")
def stock_level(sku: str):
    return {"sku": sku, "available": INVENTORY.available(sku)}


@app.get("/stats")
def stats():
    top = ORDERS.top_customers(1)
    return {"orders": len(ORDERS.orders), "events": LOG.counts(), "top": top[0][0] if top else None}
