# Ticket desk: FastAPI routes with a header-token dependency chain (current_user -> require_admin), a pydantic body / response model, query params with defaults and a bounded Query, HTTPException with dict detail, a module dict as repository; exercised with TestClient, plus tickets from JSON in os.environ validated before posting
"""Hypothesis: every status code follows from the program text: 201 for a valid body and token,
422 for a short title / a missing header / a limit over 50, 401 for an unknown token, 403 for a
non-admin delete, 404 for a second delete (dict detail with keys error, id). The stored record
gets `urgent` only for priority <= 2; the response model makes `urgent` a definite key either
way (False by default). Search results are exact over the literal sequence of requests. Tickets
from input that pass TicketIn are posted with a valid token: every such post is a 201; the input
must give `title` (the other fields have defaults)."""
import json
import os
from typing import Optional

from fastapi import Depends, FastAPI, Header, HTTPException, Query
from fastapi.testclient import TestClient
from pydantic import BaseModel, Field, ValidationError

USERS = {"tok-a": {"name": "ann", "role": "admin"}, "tok-b": {"name": "bo", "role": "agent"}}
TICKETS = {}


class TicketIn(BaseModel):
    title: str = Field(min_length=3)
    priority: int = Field(default=3, ge=1, le=5)
    tags: list[str] = []


class TicketOut(BaseModel):
    id: int
    title: str
    priority: int
    owner: str
    urgent: bool


def current_user(x_token: str = Header()):
    user = USERS.get(x_token)
    if user is None:
        raise HTTPException(status_code=401, detail="bad token")
    return user


def require_admin(user: dict = Depends(current_user)):
    if user["role"] != "admin":                  #@ ok role
        raise HTTPException(status_code=403, detail="admins only")
    return user


def store(data, owner):
    tid = max(TICKETS, default=0) + 1
    rec = {"id": tid, **data, "owner": owner}
    if data["priority"] <= 2:
        rec["urgent"] = True
    TICKETS[tid] = rec
    #@row rec: present={id,title,priority,tags,owner,urgent} definite={id,title,priority,tags,owner} closed
    return rec


def to_out(rec):
    return {"id": rec["id"], "title": rec["title"], "priority": rec["priority"], "owner": rec["owner"], "urgent": rec.get("urgent", False)}


app = FastAPI()


@app.post("/tickets", response_model=TicketOut, status_code=201)
def create(body: TicketIn, user: dict = Depends(current_user)):
    rec = store(body.model_dump(), user["name"])
    return to_out(rec)


@app.get("/tickets")
def search(min_priority: int = 1, owner: Optional[str] = None, limit: int = Query(default=10, le=50)):
    hits = [t for t in TICKETS.values() if t["priority"] >= min_priority]
    if owner is not None:
        hits = [t for t in hits if t["owner"] == owner]
    return {"total": len(hits), "ids": [t["id"] for t in hits[:limit]]}


@app.delete("/tickets/{tid}")
def close(tid: int, admin: dict = Depends(require_admin)):
    rec = TICKETS.pop(tid, None)
    if rec is None:
        raise HTTPException(status_code=404, detail={"error": "not_found", "id": tid})
    return {"closed": tid, "by": admin["name"]}


def submit_all(client, items, headers):
    for item in items:
        try:
            body = TicketIn.model_validate(item)
        except ValidationError:
            continue
        code = client.post("/tickets", json=body.model_dump(), headers=headers).status_code  #@value code: exactly={201}


if __name__ == "__main__":
    client = TestClient(app)
    A = {"x-token": "tok-a"}
    B = {"x-token": "tok-b"}
    r1 = client.post("/tickets", json={"title": "printer jam", "priority": 2}, headers=B)
    c1 = r1.status_code
    t1 = r1.json()
    #@row t1: present={id,title,priority,owner,urgent} definite={id,title,priority,owner,urgent} absent={tags} closed
    own = t1["owner"]
    urg = t1["urgent"]                           #@value urg: exactly={True}
    t2 = client.post("/tickets", json={"title": "vpn down", "tags": ["net"]}, headers=A).json()
    p2 = t2["priority"]
    u2 = t2["urgent"]                            #@value u2: exactly={False}
    short = client.post("/tickets", json={"title": "x"}, headers=A).status_code      #@value short: exactly={422}
    noauth = client.post("/tickets", json={"title": "abc"}, headers={"x-token": "nope"}).status_code  #@value noauth: exactly={401}
    nohdr = client.post("/tickets", json={"title": "abc"}).status_code

    s = client.get("/tickets").json()
    #@row s: present={total,ids} definite={total,ids} closed
    tot = s["total"]                             #@value tot: exactly={2}
    ids = tuple(client.get("/tickets", params={"min_priority": 3}).json()["ids"])   #@value ids: exactly={(2,)}
    mine = client.get("/tickets", params={"owner": "bo"}).json()["total"]
    toolong = client.get("/tickets", params={"limit": 99}).status_code               #@value toolong: exactly={422}
    forbid = client.delete("/tickets/1", headers=B).status_code                      #@value forbid: exactly={403}
    done = client.delete("/tickets/1", headers=A).json()
    #@row done: present={closed,by} definite={closed,by} closed
    by = done["by"]
    gone = client.delete("/tickets/1", headers=A)
    gcode = gone.status_code                     #@value gcode: exactly={404}
    gd = gone.json()["detail"]
    #@row gd: present={error,id} definite={error,id} closed
    left = tuple(TICKETS)

    bulk = json.loads(os.environ.get("TICKETS_JSON", '[{"title": "disk full", "priority": 1}, {"title": "font", "tags": ["ui"]}, {"title": "no"}]'))
    #@schema json.loads@L129['*']: always={title} only={title}
    submit_all(client, bulk, A)
