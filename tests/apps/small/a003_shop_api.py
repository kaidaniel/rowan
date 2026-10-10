# Shop API: FastAPI routes over a repository dict and a service function, pydantic request / response models, path / query params, HTTPException, a raw dict import, exercised with TestClient
from typing import Optional

from fastapi import Depends, FastAPI, HTTPException
from fastapi.testclient import TestClient
from pydantic import BaseModel, Field


class ProductIn(BaseModel):
    name: str
    price: float = Field(gt=0)
    stock: int = 0
    tags: list[str] = []


class ProductOut(BaseModel):
    id: int
    name: str
    price: float
    in_stock: bool


class OrderIn(BaseModel):
    product_id: int
    qty: int = Field(default=1, ge=1)
    note: Optional[str] = None


class Repo:
    def __init__(self):
        self.products = {}
        self.orders = []
        self.seq = 0

    def add(self, data):
        self.seq += 1
        row = {"id": self.seq, **data}
        #@row row: present={id,name,price,stock,tags} definite={id,name,price,stock,tags} closed
        self.products[self.seq] = row
        return row

    def get(self, pid):
        if pid not in self.products:
            raise HTTPException(status_code=404, detail="no such product")
        return self.products[pid]


REPO = Repo()


def get_repo():
    return REPO


def to_out(row):
    out = {"id": row["id"], "name": row["name"], "price": row["price"], "in_stock": row["stock"] > 0}  #@ ok id ; ok name ; ok price ; ok stock
    return out


def place_order(repo, order):
    product = repo.get(order.product_id)
    if product["stock"] < order.qty:             #@ ok stock
        raise HTTPException(status_code=409, detail={"error": "insufficient_stock", "available": product["stock"]})
    product["stock"] -= order.qty
    rec = {"product_id": product["id"], "qty": order.qty, "total": round(product["price"] * order.qty, 2)}
    if order.note:
        rec["note"] = order.note
    _ = 0  #@row rec: present={product_id,qty,total,note} definite={product_id,qty,total} closed
    repo.orders.append(rec)
    return rec


app = FastAPI()


@app.post("/products", response_model=ProductOut, status_code=201)
def create_product(body: ProductIn, repo: Repo = Depends(get_repo)):
    _ = 0  #@row body: present={name,price,stock,tags} definite={name,price,stock,tags} closed
    row = repo.add(body.model_dump())
    return to_out(row)


@app.get("/products/{pid}", response_model=ProductOut)
def read_product(pid: int, repo: Repo = Depends(get_repo)):
    return to_out(repo.get(pid))


@app.get("/products")
def list_products(min_price: float = 0.0, tag: Optional[str] = None, repo: Repo = Depends(get_repo)):
    rows = [r for r in repo.products.values() if r["price"] >= min_price]
    if tag is not None:
        rows = [r for r in rows if tag in r["tags"]]
    return {"count": len(rows), "items": [to_out(r) for r in rows]}


@app.post("/orders", status_code=201)
def create_order(order: OrderIn, repo: Repo = Depends(get_repo)):
    rec = place_order(repo, order)
    return {"order": rec, "remaining": repo.get(order.product_id)["stock"]}


@app.post("/import")
def import_products(payload: dict, repo: Repo = Depends(get_repo)):
    created = []
    for raw in payload["products"]:              #@ input products
        item = ProductIn(name=raw["title"], price=raw["price"], stock=raw.get("qty", 0))
        created.append(repo.add(item.model_dump())["id"])
    return {"created": created}
#@schema param import_products.payload: always={products} only={products}
#@schema param import_products.payload['products']['*']: always={title,price} only={title,price}


if __name__ == "__main__":
    client = TestClient(app)
    p1 = client.post("/products", json={"name": "pen", "price": 1.5, "stock": 10, "tags": ["office"]}).json()
    #@row p1: present={id,name,price,in_stock} definite={id,name,price,in_stock} absent={stock,tags} closed
    pid = p1["id"]                               #@value pid: exactly={1}
    client.post("/products", json={"name": "mug", "price": 6.0, "tags": ["kitchen", "office"]})
    code = client.post("/products", json={"name": "free", "price": 0}).status_code   #@value code: exactly={422}
    o = client.post("/orders", json={"product_id": 1, "qty": 3, "note": "gift"}).json()
    #@row o: present={order,remaining} definite={order,remaining} closed
    order = o["order"]
    #@row order: present={product_id,qty,total,note} definite={product_id,qty,total,note} closed
    rem = o["remaining"]                         #@value rem: exactly={7}
    total = order["total"]                       #@value total: exactly={4.5}
    conflict = client.post("/orders", json={"product_id": 2})
    ccode = conflict.status_code                 #@value ccode: exactly={409}
    detail = conflict.json()["detail"]           #@row detail: present={error,available} definite={error,available} closed
    nf = client.get("/products/99").status_code  #@value nf: exactly={404}
    mug = client.get("/products/2").json()
    instock = mug["in_stock"]                    #@value instock: exactly={False}
    office = client.get("/products", params={"tag": "office"}).json()
    cnt = office["count"]                        #@value cnt: exactly={2}
    dear = client.get("/products", params={"min_price": 5}).json()["count"]   #@value dear: exactly={1}
    imp = client.post("/import", json={"products": [{"title": "ink", "price": 3.0, "qty": 2}, {"title": "pad", "price": 2.0, "colour": "red"}]}).json()
    created = tuple(imp["created"])              #@value created: exactly={(3,4)}
    try:
        client.post("/import", json={"items": []})
    except KeyError:
        pass
    try:
        p1["stock"]                              #@ missing stock
    except KeyError:
        pass
