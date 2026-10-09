# Query parameters: defaults, Optional, Query(...) with defaults; values flow into a helper
from typing import Optional

from fastapi import FastAPI, Query
from fastapi.testclient import TestClient

app = FastAPI()


def page_bounds(page, size):
    start = (page - 1) * size
    bounds = {"start": start, "stop": start + size}
    #@row bounds: present={start,stop} definite={start,stop} closed
    return bounds


@app.get("/items")
def list_items(page: int = 1, size: int = Query(10, le=100), q: Optional[str] = None):
    b = page_bounds(page, size)
    #@row b: present={start,stop} definite={start,stop} closed
    out = {"page": page, "start": b["start"]}           #@ ok start
    if q is not None:
        out["q"] = q.lower()
    _ = 0  #@row out: present={page,start,q} definite={page,start} closed
    out["stop"] = b["stop"]                              #@ ok stop
    return out


@app.get("/fixed")
def fixed(flag: bool = False):
    n = 3 if flag else 4
    #@value n: exactly={3,4}
    return {"n": n, "label": "x" * n}


if __name__ == "__main__":
    client = TestClient(app)
    r = client.get("/items")
    assert r.json() == {"page": 1, "start": 0, "stop": 10}, r.json()
    r = client.get("/items", params={"page": 3, "size": 5, "q": "ABC"})
    assert r.json()["q"] == "abc"
    r = client.get("/items", params={"size": 500})
    assert r.status_code == 422
    r = client.get("/items", params={"q": "", "page": 2})
    assert r.json()["start"] == 10
    assert client.get("/fixed").json()["n"] == 4
    assert client.get("/fixed", params={"flag": "true"}).json()["n"] == 3
    assert client.get("/fixed", params={"flag": "no"}).json()["n"] == 4
