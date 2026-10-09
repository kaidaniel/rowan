# Profile service with auth: a middleware putting a request context on request.state, cookie / header sessions resolved by nested dependencies, an AuthError handler, whitelisted dict updates, router dependencies, response_model_include
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, Cookie, Depends, FastAPI, Header, HTTPException, Request
from fastapi.responses import JSONResponse
from fastapi.testclient import TestClient
from pydantic import BaseModel

# ---------------------------------------------------------------- data

USERS = {
    "ann": {"name": "ann", "email": "ann@x.org", "roles": ["admin"], "bio": "hi", "city": "Oslo"},
    "bob": {"name": "bob", "email": "bob@x.org", "roles": []},
}
SESSIONS = {"s-ann": "ann", "s-bob": "bob"}
API_KEYS = {"k-ann": "ann"}
EDITABLE = ("email", "bio", "city")
REQUEST_LOG = []


class Profile(BaseModel):
    name: str
    email: str
    roles: List[str] = []
    bio: Optional[str] = None
    city: Optional[str] = None


@dataclass
class Principal:
    name: str
    via: str
    roles: List[str] = field(default_factory=list)


class AuthError(Exception):
    def __init__(self, reason, status=401):
        self.reason = reason
        self.status = status


# ---------------------------------------------------------------- middleware

def request_context(request):
    ctx = {"path": request.url.path, "method": request.method}
    rid = request.headers.get("x-request-id")
    if rid is not None:
        ctx["request_id"] = rid
    _ = 0  #@row ctx: present={path,method,request_id} definite={path,method} closed
    return ctx


app = FastAPI()


@app.middleware("http")
async def context_middleware(request: Request, call_next):
    request.state.ctx = request_context(request)
    response = await call_next(request)
    entry = {**request.state.ctx, "status": response.status_code}
    #@row entry: present={path,method,request_id,status} definite={path,method,status} closed
    REQUEST_LOG.append(entry)
    if "request_id" in entry:
        response.headers["X-Request-Id"] = entry["request_id"]  #@ ok request_id
    return response


@app.exception_handler(AuthError)
async def auth_error(request: Request, exc: AuthError):
    _ = 0  #@row exc: present={reason,status} definite={reason,status} closed
    ctx = request.state.ctx
    #@row ctx: present={path,method,request_id} definite={path,method} closed
    return JSONResponse(status_code=exc.status, content={"error": exc.reason, "path": ctx["path"]})  #@ ok path


# ---------------------------------------------------------------- dependencies

def session_user(session: Optional[str] = Cookie(None)):
    if session is None:
        return None
    return SESSIONS.get(session)


def key_user(x_api_key: Optional[str] = Header(None)):
    if x_api_key is None:
        return None
    name = API_KEYS.get(x_api_key)
    if name is None:
        raise AuthError("bad api key")
    return name


def principal(by_cookie: Optional[str] = Depends(session_user), by_key: Optional[str] = Depends(key_user)) -> Principal:
    name = by_key or by_cookie
    if name is None:
        raise AuthError("login required")
    rec = USERS[name]                                #@ ok ann ; ok bob
    #@row rec: present={name,email,roles,bio,city} definite={name,email,roles} closed
    p = Principal(name, "key" if by_key else "cookie", list(rec["roles"]))  #@ ok roles
    #@row p: present={name,via,roles} definite={name,via,roles} closed
    return p


def admin(p: Principal = Depends(principal)) -> Principal:
    if "admin" not in p.roles:
        raise AuthError("admin only", status=403)
    return p


# ---------------------------------------------------------------- helpers

def clean_update(body):
    changes = {k: body[k] for k in EDITABLE if k in body}
    #@row changes: present={email,bio,city} closed
    ignored = sorted(k for k in body if k not in EDITABLE)
    if "email" in changes and "@" not in str(changes["email"]):  #@ ok email
        raise HTTPException(status_code=422, detail={"field": "email", "error": "invalid"})
    return changes, ignored


def apply_update(name, changes):
    rec = USERS[name]
    rec.update(changes)
    #@row rec: present={name,email,roles,bio,city} definite={name,email,roles} closed
    return rec


# ---------------------------------------------------------------- routes

me_router = APIRouter(prefix="/me")
admin_router = APIRouter(prefix="/admin", dependencies=[Depends(admin)])


@me_router.get("", response_model=Profile, response_model_exclude_none=True)
def read_me(p: Principal = Depends(principal)):
    return USERS[p.name]


@me_router.patch("")
def update_me(body: Dict[str, Any], p: Principal = Depends(principal)):
    changes, ignored = clean_update(body)
    rec = apply_update(p.name, changes)
    out = {"changed": sorted(changes), "ignored": ignored, "via": p.via}
    #@row out: present={changed,ignored,via} definite={changed,ignored,via} closed
    return out
#@schema param update_me.body: only={}


@me_router.get("/card", response_model=Profile, response_model_include={"name", "city"})
def card(p: Principal = Depends(principal)):
    return USERS[p.name]


@admin_router.post("/users/{name}/roles")
def set_roles(name: str, body: Dict[str, Any], request: Request):
    if name not in USERS:
        raise HTTPException(status_code=404)
    roles = body["roles"]                            #@ input roles
    USERS[name]["roles"] = sorted(set(roles))
    ctx = request.state.ctx
    return {"name": name, "roles": USERS[name]["roles"], "path": ctx["path"]}  #@ ok path
#@schema param set_roles.body: always={roles} only={roles}


app.include_router(me_router)
app.include_router(admin_router)


if __name__ == "__main__":
    client = TestClient(app)
    r = client.get("/me")
    assert r.status_code == 401 and r.json() == {"error": "login required", "path": "/me"}
    client.cookies.set("session", "s-bob")
    me = client.get("/me", headers={"X-Request-Id": "r1"})
    assert me.headers["x-request-id"] == "r1"
    bob = me.json()
    #@row bob: present={name,email,roles,bio,city} definite={name,email,roles} absent={bio,city} closed
    up = client.patch("/me", json={"bio": "new", "admin": True, "name": "eve"}).json()
    assert up == {"changed": ["bio"], "ignored": ["admin", "name"], "via": "cookie"}, up
    assert client.patch("/me", json={"email": "nope"}).status_code == 422
    assert client.patch("/me", json={}).json()["changed"] == []
    card_bob = client.get("/me/card").json()
    #@row card_bob: present={name,city} definite={name,city} absent={email,roles,bio} closed
    assert client.post("/admin/users/bob/roles", json={"roles": ["x"]}).status_code == 403
    client.cookies.clear()

    key = {"X-API-Key": "k-ann"}
    ann = client.get("/me", headers=key).json()
    #@row ann: present={name,email,roles,bio,city} definite={name,email,roles,bio,city} closed
    assert client.get("/me", headers={"X-API-Key": "k-zzz"}).json() == {"error": "bad api key", "path": "/me"}
    assert client.patch("/me", json={"city": "Rome", "email": "a@y.org"}, headers=key).json()["via"] == "key"
    r = client.post("/admin/users/bob/roles", json={"roles": ["dev", "dev", "ops"]}, headers=key)
    assert r.json() == {"name": "bob", "roles": ["dev", "ops"], "path": "/admin/users/bob/roles"}, r.json()
    assert client.post("/admin/users/zed/roles", json={"roles": []}, headers=key).status_code == 404
    try:
        client.post("/admin/users/bob/roles", json={"role": "x"}, headers=key)
    except KeyError:
        pass
    statuses = [e["status"] for e in REQUEST_LOG]
    assert statuses[:3] == [401, 200, 200], statuses
    first = REQUEST_LOG[0]
    #@row first: present={path,method,status} definite={path,method,status} absent={request_id} closed
