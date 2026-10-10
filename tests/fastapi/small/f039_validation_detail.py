"""A request body that breaks the model is a 422 whose detail lists pydantic's errors with their
location prefixed by "body": the error type and location are known, the message too where
rowan knows pydantic's text; `ctx` only for constrained fields."""
from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel, Field

app = FastAPI()


class OrderIn(BaseModel):
    item: str
    qty: int = Field(gt=0)


@app.post("/orders")
def create(order: OrderIn):
    return {"ok": True}


client = TestClient(app)

low = client.post("/orders", json={"item": "tea", "qty": 0})
low_code = low.status_code
#@value low_code: exactly={422}
low_detail = low.json()["detail"]
n_low = len(low_detail)
#@value n_low: exactly={1}
e0 = low_detail[0]
#@row e0: present={type, loc, msg, input, ctx} definite={type, loc, msg, input, ctx} closed
t0 = e0["type"]
#@value t0: exactly={"greater_than"}
loc0 = tuple(e0["loc"])
#@value loc0: exactly={("body", "qty")}

missing = client.post("/orders", json={"qty": 1})
m0 = missing.json()["detail"][0]
#@row m0: present={type, loc, msg, input} definite={type, loc, msg, input} closed
tm = m0["type"]
#@value tm: exactly={"missing"}
locm = tuple(m0["loc"])
#@value locm: exactly={("body", "item")}
