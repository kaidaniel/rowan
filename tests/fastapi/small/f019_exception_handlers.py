# Custom exception classes raised in helpers and turned into responses by @app.exception_handler (also one for KeyError)
from typing import Any, Dict

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from fastapi.testclient import TestClient

app = FastAPI()

SEEN = []


class NotFound(Exception):
    def __init__(self, kind, key):
        self.kind = kind
        self.key = key


class Conflict(Exception):
    def __init__(self, key, current):
        self.key = key
        self.current = current
        self.retry = current.get("version", 0) + 1


@app.exception_handler(NotFound)
async def not_found(request: Request, exc: NotFound):
    #@row exc: present={kind,key} definite={kind,key} closed
    body = {"error": f"{exc.kind} not found", "key": exc.key}
    SEEN.append(body)
    return JSONResponse(status_code=404, content=body)


@app.exception_handler(Conflict)
async def conflict(request: Request, exc: Conflict):
    #@row exc: present={key,current,retry} definite={key,current,retry} closed
    return JSONResponse(status_code=409, content={"key": exc.key, "retry": exc.retry})


@app.exception_handler(KeyError)
async def key_error(request: Request, exc: KeyError):
    return JSONResponse(status_code=400, content={"missing": exc.args[0]})


DOCS = {"d1": {"title": "a", "version": 3}}


def load(key):
    if key not in DOCS:
        raise NotFound("doc", key)
    return DOCS[key]


@app.put("/docs/{key}")
def put_doc(key: str, body: Dict[str, Any]):
    title = body["title"]                            #@ input title
    if key in DOCS and body.get("version") != DOCS[key]["version"]:
        raise Conflict(key, DOCS[key])
    DOCS[key] = {"title": title, "version": body.get("version", 0) + 1}
    return DOCS[key]
#@schema param put_doc.body: always={title} only={title}


@app.get("/docs/{key}")
def get_doc(key: str):
    d = load(key)
    #@row d: present={title,version} definite={title,version} closed
    return d


if __name__ == "__main__":
    client = TestClient(app)
    r = client.get("/docs/zz")
    assert r.status_code == 404 and r.json() == {"error": "doc not found", "key": "zz"}
    assert client.get("/docs/d1").status_code == 200
    r = client.put("/docs/d1", json={"title": "b"})
    assert r.status_code == 409 and r.json() == {"key": "d1", "retry": 4}
    assert client.put("/docs/d1", json={"title": "b", "version": 3}).json() == {"title": "b", "version": 4}
    assert client.put("/docs/d2", json={"title": "new"}).json() == {"title": "new", "version": 1}
    # the KeyError handler turns the missing key into a 400 response: nothing reaches the client
    r = client.put("/docs/d3", json={"name": "x"})
    assert r.status_code == 400 and r.json() == {"missing": "title"}
    last = SEEN[-1]
    #@row last: present={error,key} definite={error,key} closed
