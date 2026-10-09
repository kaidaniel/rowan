# @app.middleware("http"): the middleware stores a dict on request.state, the handler reads it, the middleware adds headers
from fastapi import FastAPI, Request
from fastapi.testclient import TestClient

app = FastAPI()

LOG = []


def trace_of(request):
    t = {"path": request.url.path, "method": request.method}
    tenant = request.headers.get("x-tenant")
    if tenant:
        t["tenant"] = tenant
    #@row t: present={path,method,tenant} definite={path,method} closed
    return t


@app.middleware("http")
async def tracer(request: Request, call_next):
    request.state.trace = trace_of(request)
    response = await call_next(request)
    entry = dict(request.state.trace, status=response.status_code)
    #@row entry: present={path,method,tenant,status} definite={path,method,status} closed
    LOG.append(entry)
    response.headers["X-Path"] = entry["path"]       #@ ok path
    return response


@app.get("/hello/{name}")
def hello(name: str, request: Request):
    t = request.state.trace
    #@row t: present={path,method,tenant} definite={path,method} closed
    who = t.get("tenant", "public")
    return {"msg": f"hello {name}", "tenant": who}


@app.post("/echo")
async def echo(request: Request):
    t = request.state.trace
    return {"method": t["method"], "tenant": t["tenant"]}  #@ ok method ; possibly tenant


if __name__ == "__main__":
    client = TestClient(app)
    r = client.get("/hello/ann")
    assert r.headers["x-path"] == "/hello/ann" and r.json()["tenant"] == "public"
    r = client.get("/hello/bob", headers={"X-Tenant": "acme"})
    assert r.json()["tenant"] == "acme"
    assert client.post("/echo", headers={"X-Tenant": "t1"}).json() == {"method": "POST", "tenant": "t1"}
    try:
        client.post("/echo")
    except KeyError:
        pass
    assert client.get("/nowhere").status_code == 404
    last = LOG[-1]
    #@row last: present={path,method,status} definite={path,method,status} closed
