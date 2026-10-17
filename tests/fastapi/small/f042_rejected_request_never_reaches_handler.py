"""A request whose body fails validation (`qty=0` breaks `gt=0`, "XL" is not a size) is a 422
answered before the handler runs: the handler's parameter hover lists only the requests that
reach it (lines 32 and 33) and any request the framework may pass (its own run), not the
rejected one (line 34). vscode-rowan/test.js checks the hover."""
from typing import Literal

from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient
from pydantic import BaseModel, Field

app = FastAPI()
ORDERS = {}


class OrderIn(BaseModel):
    item: str
    qty: int = Field(gt=0)
    size: Literal["S", "M", "L"] = "M"


@app.post("/orders/{oid}")
def create(oid: int, order: OrderIn):
    if oid in ORDERS:
        raise HTTPException(status_code=409, detail="exists")
    ORDERS[oid] = order
    total = order.qty * 2.5
    #@value total: includes={10.0}
    return {"id": oid, "item": order.item, "total": total}


client = TestClient(app)
ok = client.post("/orders/1", json={"item": "tea", "qty": "4"})
dup = client.post("/orders/1", json={"item": "tea", "qty": 1})
bad = client.post("/orders/2", json={"item": "tea", "qty": 0, "size": "XL"})
codes = (ok.status_code, dup.status_code, bad.status_code)
#@value codes: exactly={(200, 409, 422)}
