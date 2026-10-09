# Nested dependencies: get_admin -> get_user -> get_db (header and cache: get_db runs once per request even when used twice)
from fastapi import Depends, FastAPI, Header, HTTPException
from fastapi.testclient import TestClient

app = FastAPI()

CALLS = []


def get_db():
    CALLS.append("db")
    db = {
        "users": {"ann": {"name": "ann", "admin": True}, "bob": {"name": "bob"}},
        "settings": {"theme": "dark"},
    }
    return db


def get_user(x_user: str = Header(...), db: dict = Depends(get_db)):
    users = db["users"]                              #@ ok users
    user = users.get(x_user)
    if user is None:
        raise HTTPException(status_code=401, detail="unknown user")
    #@row user: present={name,admin} definite={name} closed
    return user


def get_admin(user: dict = Depends(get_user)):
    if not user.get("admin"):
        raise HTTPException(status_code=403, detail="admins only")
    #@row user: present={name,admin} definite={name,admin} closed
    return user


@app.get("/settings")
def settings(admin: dict = Depends(get_admin), db: dict = Depends(get_db)):
    #@row db: present={users,settings} definite={users,settings} closed
    out = dict(db["settings"])                       #@ ok settings
    out["by"] = admin["name"]                        #@ ok name
    #@row out: present={theme,by} definite={theme,by} closed
    return out


@app.get("/me")
def me(user: dict = Depends(get_user)):
    return {"name": user["name"], "admin": user["admin"]}  #@ possibly admin


if __name__ == "__main__":
    client = TestClient(app)
    assert client.get("/settings", headers={"X-User": "ann"}).json() == {"theme": "dark", "by": "ann"}
    n = len(CALLS)                                   #@value n: exactly={1}
    assert client.get("/settings", headers={"X-User": "bob"}).status_code == 403
    assert client.get("/settings", headers={"X-User": "cyd"}).status_code == 401
    assert client.get("/settings").status_code == 422
    # (first written exactly={3}: FastAPI solves get_user's sub-dependency get_db before it
    # validates get_user's own header parameter, so the 422 request ran get_db too)
    m = len(CALLS)                                   #@value m: exactly={4}
    for who in ("ann", "bob"):
        try:
            client.get("/me", headers={"X-User": who})
        except KeyError:
            pass
