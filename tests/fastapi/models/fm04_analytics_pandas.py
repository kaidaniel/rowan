# Analytics ingestion: raw events posted as JSON (request.json(), json.loads(request.body())), validated by hand, stored, turned into pandas frames for reports (groupby / agg / merge), Counter summaries
import json
from collections import Counter
from typing import Any, Dict, List, Optional

import pandas as pd
from fastapi import FastAPI, HTTPException, Query, Request
from fastapi.testclient import TestClient

app = FastAPI()

EVENTS: List[Dict[str, Any]] = []
USERS = pd.DataFrame({"user": ["u1", "u2", "u3"], "plan": ["free", "pro", "pro"], "country": ["NO", "SE", "NO"]})
#@row USERS: present={user,plan,country} definite={user,plan,country} closed
KNOWN_KINDS = {"view": 1, "click": 2, "buy": 10}


# ---------------------------------------------------------------- validation

def normalize_event(raw):
    kind = raw["kind"]                               #@ input kind
    user = raw["user"]                               #@ input user
    if kind not in KNOWN_KINDS:
        raise HTTPException(status_code=422, detail={"error": "unknown kind", "kind": kind})
    ev = {"kind": kind, "user": user, "weight": KNOWN_KINDS[kind], "value": float(raw.get("value", 0))}  #@ ok view ; ok click ; ok buy
    if "props" in raw:
        ev["n_props"] = len(raw["props"])            #@ ok props
    _ = 0  #@row ev: present={kind,user,weight,value,n_props} definite={kind,user,weight,value} closed
    #@row raw: definite={kind,user} open
    return ev


def store(events):
    for ev in events:
        EVENTS.append(ev)
    return len(EVENTS)


# ---------------------------------------------------------------- frames

def events_frame():
    df = pd.DataFrame(EVENTS, columns=["kind", "user", "weight", "value"])
    #@row df: present={kind,user,weight,value} definite={kind,user,weight,value} closed
    df["score"] = df["weight"] * (1 + df["value"])  #@ ok weight ; ok value
    return df


def per_user(df):
    g = df.groupby("user", as_index=False).agg(events=("kind", "count"), score=("score", "sum"), spend=("value", "sum"))
    #@row g: present={user,events,score,spend} definite={user,events,score,spend} closed
    joined = g.merge(USERS, on="user", how="left")
    #@row joined: present={user,events,score,spend,plan,country} definite={user,events,score,spend,plan,country} closed
    return joined


def per_plan(joined):
    p = joined.groupby("plan").agg({"score": "sum", "events": "sum"}).reset_index()
    #@row p: present={plan,score,events} definite={plan,score,events} closed
    return p


# ---------------------------------------------------------------- routes

@app.post("/events")
async def ingest_one(request: Request):
    raw = await request.json()
    ev = normalize_event(raw)
    n = store([ev])
    return {"stored": n, "kind": ev["kind"]}         #@ ok kind


@app.post("/events/batch")
async def ingest_batch(request: Request):
    payload = json.loads(await request.body())
    batch = payload["events"]                        #@ input events
    #@row payload: definite={events} open
    evs = [normalize_event(raw) for raw in batch]
    n = store(evs)
    kinds = Counter(e["kind"] for e in evs)          #@ ok kind
    return {"stored": n, "kinds": dict(kinds), "source": payload.get("source", "batch")}
#@schema json.loads@L74: always={events} only={events}
#@schema json.loads@L74['events']['*']: always={kind,user} only={kind,user}


@app.get("/report/users")
def report_users(min_events: int = Query(0, ge=0), country: Optional[str] = None):
    df = events_frame()
    joined = per_user(df)
    if country is not None:
        joined = joined[joined["country"] == country]  #@ ok country
    joined = joined[joined["events"] >= min_events]  #@ ok events
    recs = joined[["user", "events", "score", "plan"]].to_dict("records")
    for r in recs:
        #@row r: present={user,events,score,plan} definite={user,events,score,plan} closed
        r["score"] = round(r["score"], 2)            #@ ok score
    return recs


@app.get("/report/plans")
def report_plans():
    p = per_plan(per_user(events_frame()))
    out = {row["plan"]: {"score": round(row["score"], 2), "events": int(row["events"])} for row in p.to_dict("records")}
    return out


@app.get("/report/kinds")
def report_kinds():
    counts = Counter(e["kind"] for e in EVENTS)
    top = counts.most_common(1)
    res = {"distinct": len(counts), "top": top[0][0] if top else None}
    #@row res: present={distinct,top} definite={distinct,top} closed
    return res


@app.get("/users/{user}/summary")
def user_summary(user: str):
    df = events_frame()
    mine = df[df["user"] == user]                    #@ ok user
    #@row mine: present={kind,user,weight,value,score} definite={kind,user,weight,value,score} closed
    if mine.empty:
        raise HTTPException(status_code=404, detail={"error": "no events", "user": user})
    # kinds were checked against KNOWN_KINDS on the way in: the keys are among its keys
    by_kind = mine.groupby("kind")["score"].sum().round(2).to_dict()
    #@todo row by_kind: present={view,click,buy} closed  (column values from a list of records into the frame and its groupby keys)
    known = USERS[USERS["user"] == user]
    info = known.iloc[0].to_dict() if len(known) else {"user": user}
    #@row info: present={user,plan,country} definite={user} closed
    summary = {"user": user, "by_kind": by_kind, "plan": info.get("plan", "unknown")}
    #@row summary: present={user,by_kind,plan} definite={user,by_kind,plan} closed
    return summary


if __name__ == "__main__":
    client = TestClient(app)
    assert client.post("/events", json={"kind": "view", "user": "u1"}).json() == {"stored": 1, "kind": "view"}
    assert client.post("/events", json={"kind": "buy", "user": "u2", "value": 3, "props": {"sku": "x"}}).status_code == 200
    assert client.post("/events", json={"kind": "jump", "user": "u1"}).status_code == 422
    for bad in [{"kind": "view"}, {"user": "u3"}]:
        try:
            client.post("/events", json=bad)
        except KeyError:
            pass
    b = client.post("/events/batch", json={"events": [{"kind": "click", "user": "u3"}, {"kind": "view", "user": "u2", "extra": 1}], "source": "sdk"}).json()
    assert b == {"stored": 4, "kinds": {"click": 1, "view": 1}, "source": "sdk"}, b
    assert client.post("/events/batch", json={"events": []}).json()["stored"] == 4
    for bad in [{"evts": []}, {"events": [{"kind": "view"}]}]:
        try:
            client.post("/events/batch", json=bad)
        except KeyError:
            pass

    rows = client.get("/report/users").json()
    assert [r["user"] for r in rows] == ["u1", "u2", "u3"], rows
    first = rows[0]
    #@row first: present={user,events,score,plan} definite={user,events,score,plan} closed
    assert [r["user"] for r in client.get("/report/users", params={"country": "NO"}).json()] == ["u1", "u3"]
    assert [r["user"] for r in client.get("/report/users", params={"min_events": 2}).json()] == ["u2"]
    plans = client.get("/report/plans").json()
    #@row plans: present={free,pro} closed
    assert plans["pro"]["events"] == 3
    k = client.get("/report/kinds").json()
    #@row k: present={distinct,top} definite={distinct,top} closed
    assert k == {"distinct": 3, "top": "view"}, k
    s2 = client.get("/users/u2/summary").json()
    #@todo row s2: present={user,by_kind,plan} definite={user,by_kind,plan} closed  (needs the frame's rows: mine.empty decides the 404 path)
    assert s2["by_kind"] == {"buy": 40.0, "view": 1.0} and s2["plan"] == "pro", s2
    assert client.get("/users/u9/summary").status_code == 404
    client.post("/events", json={"kind": "click", "user": "u9"})
    assert client.get("/users/u9/summary").json()["plan"] == "unknown"
