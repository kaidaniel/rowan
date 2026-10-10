"""Query / Path constraints (`Annotated[int, Query(ge=1, le=10)]`, `Path(gt=0)`) are 422s for
values breaking them (defaults are not validated); a background task that is a bound method
(`LOG.append`) runs after the response, before the client call returns."""
from typing import Annotated

from fastapi import BackgroundTasks, FastAPI, Path, Query
from fastapi.testclient import TestClient

app = FastAPI()
LOG = []
ITEMS = ["a", "b", "c"]


@app.get("/items")
def items(limit: Annotated[int, Query(ge=1, le=10)] = 2):
    return {"items": ITEMS[:limit]}


@app.get("/items/{iid}")
def item(iid: Annotated[int, Path(gt=0)]):
    return {"id": iid}


@app.post("/touch/{iid}", status_code=202)
def touch(iid: int, bg: BackgroundTasks):
    bg.add_task(LOG.append, iid)
    return {"queued": iid}


if __name__ == "__main__":
    c = TestClient(app)
    n = len(c.get("/items", params={"limit": 1}).json()["items"])   #@value n: exactly={1}
    d = len(c.get("/items").json()["items"])     #@value d: exactly={2}
    low = c.get("/items", params={"limit": 0}).status_code   #@value low: exactly={422}
    high = c.get("/items", params={"limit": 11}).status_code   #@value high: exactly={422}
    zero = c.get("/items/0").status_code         #@value zero: exactly={422}
    one = c.get("/items/1").json()["id"]         #@value one: exactly={1}
    s = c.post("/touch/3").status_code           #@value s: exactly={202}
    logged = tuple(LOG)                          #@value logged: exactly={(3,)}
