# A dependency parses the Authorization header into a dataclass user; handlers and a permission helper read its attributes
from dataclasses import asdict, dataclass, field
from typing import List, Optional

from fastapi import Depends, FastAPI, Header, HTTPException
from fastapi.testclient import TestClient

app = FastAPI()

TOKENS = {"t-ann": ("ann", ["admin", "dev"], "acme"), "t-bob": ("bob", ["dev"], None)}


@dataclass
class User:
    name: str
    roles: List[str] = field(default_factory=list)
    org: Optional[str] = None


def current_user(authorization: str = Header(...)) -> User:
    scheme, _, token = authorization.partition(" ")
    if scheme.lower() != "bearer" or token not in TOKENS:
        raise HTTPException(status_code=401, headers={"WWW-Authenticate": "Bearer"})
    name, roles, org = TOKENS[token]
    u = User(name, roles, org)
    #@row u: present={name,roles,org} definite={name,roles,org} closed
    return u


def require(user, role):
    if role not in user.roles:
        raise HTTPException(status_code=403, detail=f"{role} required")
    return user


@app.get("/me")
def me(user: User = Depends(current_user)):
    d = asdict(user)
    #@row d: present={name,roles,org} definite={name,roles,org} closed
    d["n_roles"] = len(user.roles)
    return d


@app.delete("/projects/{pid}")
def delete_project(pid: int, user: User = Depends(current_user)):
    admin = require(user, "admin")
    #@row admin: present={name,roles,org} definite={name,roles,org} closed
    return {"deleted": pid, "by": admin.name, "org": admin.org}


@app.get("/team")
def team(user: User = Depends(current_user)):
    return {"team": user.team}                       #@ missing team


if __name__ == "__main__":
    client = TestClient(app)
    for tok in ("t-ann", "t-bob"):
        d = client.get("/me", headers={"Authorization": f"Bearer {tok}"}).json()
        #@row d: present={name,roles,org,n_roles} definite={name,roles,org,n_roles} closed
        r = client.delete("/projects/7", headers={"Authorization": f"Bearer {tok}"})
        try:
            client.get("/team", headers={"Authorization": f"Bearer {tok}"})
        except AttributeError:
            pass
    assert client.get("/me", headers={"Authorization": "Basic x"}).status_code == 401
    assert client.get("/me").status_code == 422
