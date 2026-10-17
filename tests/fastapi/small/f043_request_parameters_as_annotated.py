# A handler's path and query parameters hold what FastAPI converts the request's text to, as
# annotated: for any request an `int` parameter is an int, a `str` a str, an `Optional[str] =
# None` None or a str, a defaulted `int` its default or an int.
#
# Expectations were written from the program text before running the engine.
from typing import Optional

from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()


@app.get("/runs/{name}")
def read_run(name: str, limit: int = 10, fields: Optional[str] = None):
    a = name
    #@general a: types={str}
    b = limit
    #@general b: includes={10} types={int}
    c = fields
    #@general c: includes={None} types={str,NoneType}
    return {"name": a, "limit": b, "fields": c}


@app.get("/items/{iid}")
def get_item(iid: int):
    d = iid + 1
    #@general d: types={int}
    return {"next": d}


if __name__ == "__main__":
    client = TestClient(app)
    r1 = client.get("/runs/a?limit=3").json()
    r2 = client.get("/runs/b?fields=x").json()
    r3 = client.get("/items/7").json()
