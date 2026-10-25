"""A handler whose call does not run (past its budget: here every call's) may raise an
HTTPException: the response's status is then not known, not the route's 200.
(Before, its raise of any class escaped the TestClient only.)

Expectations were written from the program text before running the engine."""
#@limits budget=0
from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient

app = FastAPI()
ITEMS = {"a": 1}


def find(key):
    if key not in ITEMS:
        raise HTTPException(status_code=404, detail="no such item")
    return ITEMS[key]


@app.get("/items/{key}")
def read(key: str):
    return {"key": key, "value": find(key)}


client = TestClient(app)
ok = client.get("/items/a").status_code
#@value ok: includes={200}
missing = client.get("/items/zz").status_code
#@value missing: includes={404}
