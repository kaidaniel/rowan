# response_model_exclude_none / response_model_include / response_model_exclude select the response's keys
from typing import Optional

from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class Profile(BaseModel):
    id: int
    name: str
    bio: Optional[str] = None
    city: Optional[str] = None


PROFILES = {
    1: {"id": 1, "name": "ann", "bio": "hi", "city": "Oslo"},
    2: {"id": 2, "name": "bob"},
    3: {"id": 3, "name": "cyd", "city": None, "bio": "x"},
}


def fetch(pid):
    p = Profile(**PROFILES[pid])                     #@ ok id ; ok name
    #@row p: present={id,name,bio,city} definite={id,name,bio,city} closed
    return p


@app.get("/p/{pid}", response_model=Profile, response_model_exclude_none=True)
def compact(pid: int):
    return fetch(pid)


@app.get("/p/{pid}/short", response_model=Profile, response_model_include={"id", "name"})
def short(pid: int):
    return fetch(pid)


@app.get("/p/{pid}/public", response_model=Profile, response_model_exclude={"city"})
def public(pid: int):
    return fetch(pid)


if __name__ == "__main__":
    client = TestClient(app)
    for pid in (1, 2, 3):
        c = client.get(f"/p/{pid}").json()
        #@row c: present={id,name,bio,city} definite={id,name} closed
        s = client.get(f"/p/{pid}/short").json()
        #@row s: present={id,name} definite={id,name} absent={bio,city} closed
        pub = client.get(f"/p/{pid}/public").json()
        #@row pub: present={id,name,bio} definite={id,name,bio} absent={city} closed
