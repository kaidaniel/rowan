# status_code= on the route, JSONResponse returned directly (bypasses response filtering), Response.status_code in the driver
from typing import Any, Dict

from fastapi import FastAPI, Response
from fastapi.responses import JSONResponse
from fastapi.testclient import TestClient

app = FastAPI()

DB = {1: {"id": 1, "name": "ann"}}


def not_found(kind, key):
    content = {"error": "not_found", "kind": kind, "key": key}
    #@row content: present={error,kind,key} definite={error,kind,key} closed
    return JSONResponse(status_code=404, content=content)


@app.post("/things", status_code=201)
def create(body: Dict[str, Any]):
    new_id = max(DB) + 1
    DB[new_id] = {"id": new_id, "name": body["name"]}  #@ input name
    return DB[new_id]
#@schema param create.body: always={name} only={name}


@app.get("/things/{tid}")
def read(tid: int):
    if tid not in DB:
        return not_found("thing", tid)
    rec = DB[tid]
    #@row rec: present={id,name} definite={id,name} closed
    return rec


@app.delete("/things/{tid}", status_code=204)
def delete(tid: int, response: Response):
    if DB.pop(tid, None) is None:
        response.status_code = 404
    return None


if __name__ == "__main__":
    client = TestClient(app)
    r = client.post("/things", json={"name": "pen"})
    code = r.status_code                             #@value code: exactly={201}
    try:
        client.post("/things", json={"title": "x"})
    except KeyError:
        pass
    for tid in (1, 2, 9):
        r = client.get(f"/things/{tid}")
        got = r.json()
        #@row got: present={id,name,error,kind,key} closed
    r = client.get("/things/9")
    miss = r.json()
    #@row miss: present={error,kind,key} definite={error,kind,key} closed
    nf = r.status_code                               #@value nf: exactly={404}
    assert client.delete("/things/1").status_code == 204
    assert client.delete("/things/1").status_code == 404
