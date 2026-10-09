# Path parameters (int conversion, Path(ge=), a :path converter) look records up in an in-memory repository through helper layers
from fastapi import FastAPI, HTTPException, Path
from fastapi.testclient import TestClient

app = FastAPI()

USERS = {
    1: {"id": 1, "name": "ann", "email": "ann@x.org"},
    2: {"id": 2, "name": "bob"},
}
N_USERS = len(USERS)                                 #@value N_USERS: exactly={2}
#@row USERS: present={1,2} definite={1,2} closed


def find_user(uid):
    user = USERS.get(uid)
    if user is None:
        raise HTTPException(status_code=404, detail="no such user")
    _ = 0  #@row user: present={id,name,email} definite={id,name} closed
    return user


def contact_of(user):
    email = user.get("email", "")
    name = user["name"]                              #@ ok name
    out = {"name": name, "email": email}
    #@row out: present={name,email} definite={name,email} closed
    return out


@app.get("/users/{uid}")
def get_user(uid: int = Path(ge=1)):
    user = find_user(uid)
    c = contact_of(user)
    c["id"] = user["id"]                             #@ ok id
    #@row c: present={name,email,id} definite={name,email,id} closed
    return c


@app.get("/users/{uid}/email")
def get_email(uid: int):
    user = find_user(uid)
    return {"email": user["email"]}                  #@ possibly email


@app.get("/files/{path:path}")
def get_file(path: str):
    parts = path.split("/")
    info = {"depth": len(parts), "first": parts[0], "last": parts[-1]}
    #@row info: present={depth,first,last} definite={depth,first,last} closed
    return info


if __name__ == "__main__":
    client = TestClient(app)
    for uid in (1, 2, 3):
        r = client.get(f"/users/{uid}")
        assert r.status_code == (200 if uid < 3 else 404)
    assert client.get("/users/0").status_code == 422
    assert client.get("/users/abc").status_code == 422
    for uid in (1, 2, 7):
        try:
            client.get(f"/users/{uid}/email")
        except KeyError:
            pass
    for p in ("a/b/c.txt", "readme", "x/y"):
        d = client.get(f"/files/{p}").json()
        #@row d: present={depth,first,last} definite={depth,first,last} closed
