# dict(request.query_params) is an open record: pop / get / membership on it in a helper that builds a filter spec
from fastapi import FastAPI, Request
from fastapi.testclient import TestClient

app = FastAPI()

ROWS = [{"city": "oslo", "kind": "a"}, {"city": "rome", "kind": "b"}, {"city": "oslo", "kind": "b"}]


def parse_filters(params):
    limit = int(params.pop("limit", "10"))
    order = params.pop("order", "asc")
    #@row params: absent={limit,order} open
    spec = {"limit": limit, "order": order, "where": params}
    #@row spec: present={limit,order,where} definite={limit,order,where} closed
    return spec


def run_query(spec):
    where = spec["where"]                            #@ ok where
    out = [r for r in ROWS if all(r.get(k) == v for k, v in where.items())]
    if spec["order"] == "desc":                      #@ ok order
        out.reverse()
    return out[: spec["limit"]]                      #@ ok limit


@app.get("/search")
def search(request: Request):
    params = dict(request.query_params)
    #@row params: open
    spec = parse_filters(params)
    rows = run_query(spec)
    return {"n": len(rows), "rows": rows}


@app.get("/need-city")
def need_city(request: Request):
    params = dict(request.query_params)
    city = params["city"]                            #@ possibly city
    #@row params: definite={city} open
    return {"city": city}


if __name__ == "__main__":
    client = TestClient(app)
    assert client.get("/search").json()["n"] == 3
    assert client.get("/search", params={"city": "oslo", "limit": "1"}).json()["n"] == 1
    assert client.get("/search", params={"kind": "b", "order": "desc"}).json()["rows"][0]["city"] == "oslo"
    assert client.get("/search", params={"zzz": "1"}).json()["n"] == 0
    assert client.get("/need-city", params={"city": "rome"}).json() == {"city": "rome"}
    try:
        client.get("/need-city", params={"town": "rome"})
    except KeyError:
        pass
