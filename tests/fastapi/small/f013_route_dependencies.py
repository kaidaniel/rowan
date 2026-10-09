# dependencies=[Depends(...)] on a route and on an APIRouter: run for their checks (header lookups), return values ignored
from fastapi import APIRouter, Depends, FastAPI, Header, HTTPException
from fastapi.testclient import TestClient

AUDIT = []
KEYS = {"k1": "ann", "k2": "bob"}


def verify_token(x_token: str = Header(...)):
    if x_token != "secret":
        raise HTTPException(status_code=403, detail="bad token")
    entry = {"check": "token"}
    AUDIT.append(entry)


def verify_key(x_key: str = Header()):
    owner = KEYS.get(x_key)
    if owner is None:
        raise HTTPException(status_code=403, detail="bad key")
    entry = {"check": "key", "owner": owner}
    #@row entry: present={check,owner} definite={check,owner} closed
    AUDIT.append(entry)
    return owner


router = APIRouter(prefix="/admin", dependencies=[Depends(verify_token)])


@router.get("/stats", dependencies=[Depends(verify_key)])
def stats():
    last = AUDIT[-1]
    #@row last: present={check,owner} definite={check,owner} closed
    return {"by": last["owner"]}                     #@ ok owner


@router.get("/ping")
def ping():
    last = AUDIT[-1]
    #@row last: present={check} definite={check} closed
    return {"ok": True}


app = FastAPI()
app.include_router(router)


if __name__ == "__main__":
    client = TestClient(app)
    good = {"X-Token": "secret"}
    assert client.get("/admin/stats", headers={**good, "X-Key": "k1"}).json() == {"by": "ann"}
    assert client.get("/admin/stats", headers={**good, "X-Key": "k9"}).status_code == 403
    assert client.get("/admin/stats", headers={"X-Token": "no", "X-Key": "k1"}).status_code == 403
    assert client.get("/admin/ping", headers=good).json() == {"ok": True}
    assert client.get("/admin/ping").status_code == 422
    kinds = [a["check"] for a in AUDIT]
    assert kinds == ["token", "key", "token", "token"], kinds
    first = AUDIT[0]
    #@row first: present={check} definite={check} closed
