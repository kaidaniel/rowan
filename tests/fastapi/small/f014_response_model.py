# response_model filters the returned dict: keys outside the model are dropped from the response (the handler still sees them)
from typing import Optional

from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class UserOut(BaseModel):
    id: int
    name: str
    email: Optional[str] = None


DB = {
    1: {"id": 1, "name": "ann", "password": "x", "email": "a@x.org"},
    2: {"id": 2, "name": "bob", "password": "y"},
}


def load(uid):
    if uid not in DB:
        raise HTTPException(status_code=404)
    rec = dict(DB[uid])
    #@row rec: present={id,name,password,email} definite={id,name,password} closed
    return rec


def check_password(rec, pw):
    return rec["password"] == pw                     #@ ok password


@app.get("/users/{uid}", response_model=UserOut)
def get_user(uid: int):
    rec = load(uid)
    rec["checked"] = check_password(rec, "x")
    return rec


@app.get("/raw/{uid}")
def get_raw(uid: int):
    return load(uid)


if __name__ == "__main__":
    client = TestClient(app)
    for uid in (1, 2):
        data = client.get(f"/users/{uid}").json()
        #@row data: present={id,name,email} definite={id,name,email} absent={password,checked} closed
        raw = client.get(f"/raw/{uid}").json()
        #@row raw: present={id,name,password,email} definite={id,name,password} closed
    assert client.get("/users/3").status_code == 404
