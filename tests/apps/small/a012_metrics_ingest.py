# Metrics ingestion: FastAPI handlers over a service layer (pydantic validation per record under try/except, a module dict of lists as repository), a raw dict body, a response_model that drops a key, a Literal query param with a default, a pandas frame from exclude_none dumps grouped by the requested field, an early return for an empty source; TestClient plus a batch from JSON in os.environ
"""Hypothesis: the ingest response has exactly the response model's keys (`debug` dropped),
with the accepted count and rejected indexes exact for literal batches; a batch with no valid
record is a 400 whose detail has keys error, rejected. The stats frame has `unit` only when a
stored metric had one (present-only over all calls); the grouped frame has the group column
(name or host, the Literal decides) plus count and mean; an unknown source returns early with
keys source, empty. The payload must have `metrics`, each record `name` and `value`."""
import json
import os
from typing import Literal, Optional

import pandas as pd
from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient
from pydantic import BaseModel, ValidationError


class Metric(BaseModel):
    name: str
    value: float
    host: str = "local"
    unit: Optional[str] = None


class IngestOut(BaseModel):
    source: str
    accepted: int
    rejected: list[int]


STORE = {}


def validate_batch(records):
    good, bad = [], []
    for i, raw in enumerate(records):
        try:
            good.append(Metric(**raw))
        except ValidationError:
            bad.append(i)
    return good, bad


def save(source, metrics):
    STORE.setdefault(source, []).extend(metrics)
    return len(STORE[source])


def stats(source, by):
    metrics = STORE.get(source, [])
    if not metrics:
        return {"source": source, "empty": True}
    df = pd.DataFrame([m.model_dump(exclude_none=True) for m in metrics])
    #@row df: present={name,value,host,unit} definite={name,value,host} closed
    g = df.groupby(by)["value"].agg(["count", "mean"]).reset_index()
    #@row g: present={name,host,count,mean} definite={count,mean} closed
    return {"source": source, "groups": g.to_dict(orient="records")}


app = FastAPI()


@app.post("/ingest/{source}", response_model=IngestOut)
def ingest(source: str, payload: dict):
    records = payload["metrics"]                 #@ input metrics
    if not isinstance(records, list):
        raise HTTPException(status_code=422, detail="metrics must be a list")
    good, bad = validate_batch(records)
    if not good:
        raise HTTPException(status_code=400, detail={"error": "nothing valid", "rejected": bad})
    save(source, good)
    return {"source": source, "accepted": len(good), "rejected": bad, "debug": True}
#@schema param ingest.payload: always={metrics} only={metrics}
#@schema param ingest.payload['metrics']['*']: always={name,value} only={name,value}


@app.get("/stats/{source}")
def get_stats(source: str, by: Literal["name", "host"] = "name"):
    return stats(source, by)


@app.delete("/sources/{source}")
def drop(source: str):
    if source not in STORE:
        raise HTTPException(status_code=404, detail="unknown source")
    return {"dropped": len(STORE.pop(source))}


if __name__ == "__main__":
    client = TestClient(app)
    r = client.post("/ingest/web", json={"metrics": [{"name": "cpu", "value": 0.5, "host": "w1"}, {"name": "cpu", "value": "high"}, {"name": "mem", "value": 2, "unit": "GB"}]})
    code = r.status_code                         #@value code: exactly={200}
    body = r.json()
    #@row body: present={source,accepted,rejected} definite={source,accepted,rejected} absent={debug} closed
    acc = body["accepted"]                       #@value acc: exactly={2}
    rej = tuple(body["rejected"])                #@value rej: exactly={(1,)}
    none_ok = client.post("/ingest/db", json={"metrics": [{"value": 1}, {"name": "x"}]})
    nc = none_ok.status_code                     #@value nc: exactly={400}
    nd = none_ok.json()["detail"]
    #@row nd: present={error,rejected} definite={error,rejected} closed
    st = client.get("/stats/web").json()
    #@row st: present={source,groups} definite={source,groups} absent={empty} closed
    g0 = st["groups"][0]
    #@row g0: present={name,count,mean} definite={name,count,mean} closed
    h0 = client.get("/stats/web", params={"by": "host"}).json()["groups"][0]
    #@row h0: present={host,count,mean} definite={host,count,mean} closed
    bad_by = client.get("/stats/web", params={"by": "unit"}).status_code            #@value bad_by: exactly={422}
    empty = client.get("/stats/nope").json()
    #@row empty: present={source,empty} definite={source,empty} closed
    dropped = client.delete("/sources/web").json()["dropped"]                        #@value dropped: exactly={2}
    again = client.delete("/sources/web").status_code                                #@value again: exactly={404}
    try:
        client.post("/ingest/x", json={"items": []})
    except KeyError:
        pass

    batch = json.loads(os.environ.get("METRICS_JSON", '{"metrics": [{"name": "disk", "value": 70, "host": "d1"}, {"name": "disk", "value": 80}]}'))
    #@schema json.loads@L117: always={metrics} only={metrics}
    #@schema json.loads@L117['metrics']['*']: always={name,value} only={name,value}
    try:
        client.post("/ingest/batch", json=batch)
    except KeyError:
        pass
    st2 = client.get("/stats/batch", params={"by": "host"}).json()
    #@row st2: present={source,groups,empty} definite={source} closed
