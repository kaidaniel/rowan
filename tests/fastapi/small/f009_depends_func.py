# Depends on functions that read query parameters and return dicts; the handler combines them in a helper
from typing import Optional

from fastapi import Depends, FastAPI, Query
from fastapi.testclient import TestClient

app = FastAPI()

DATA = [
    {"id": 1, "name": "apple", "active": True},
    {"id": 2, "name": "banana", "active": False},
    {"id": 3, "name": "cherry", "active": True},
]


def pagination(skip: int = 0, limit: int = Query(10, le=50)):
    page = {"skip": skip, "limit": limit}
    #@row page: present={skip,limit} definite={skip,limit} closed
    return page


def filters(q: Optional[str] = None, active: bool = True):
    f = {"active": active}
    if q:
        f["q"] = q
    #@row f: present={active,q} definite={active} closed
    return f


def apply_filters(rows, flt):
    out = []
    for r in rows:
        if r["active"] != flt["active"]:             #@ ok active
            continue
        if "q" in flt and flt["q"] not in r["name"]:  #@ ok name
            continue
        out.append(r)
    return out


@app.get("/things")
def things(page: dict = Depends(pagination), flt: dict = Depends(filters)):
    rows = apply_filters(DATA, flt)
    start = page["skip"]                             #@ ok skip
    sel = rows[start:start + page["limit"]]          #@ ok limit
    meta = {"total": len(rows), **page, **flt}
    #@row meta: present={total,skip,limit,active,q} definite={total,skip,limit,active} closed
    return {"items": sel, "meta": meta}


if __name__ == "__main__":
    client = TestClient(app)
    assert [r["id"] for r in client.get("/things").json()["items"]] == [1, 3]
    assert [r["id"] for r in client.get("/things", params={"active": "false"}).json()["items"]] == [2]
    assert [r["id"] for r in client.get("/things", params={"q": "err", "skip": 0, "limit": 1}).json()["items"]] == [3]
    assert client.get("/things", params={"limit": 99}).status_code == 422
    m = client.get("/things", params={"skip": 1}).json()["meta"]
    #@row m: present={total,skip,limit,active} definite={total,skip,limit,active} closed
