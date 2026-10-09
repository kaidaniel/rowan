# A return annotation is the response model: a dict with extra keys and an instance of a subclass are both cut to its fields
from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel

app = FastAPI()


class PublicUser(BaseModel):
    name: str
    team: str = "none"


class PrivateUser(PublicUser):
    token: str


def lookup(name):
    u = PrivateUser(name=name, token=name[::-1])
    #@row u: present={name,team,token} definite={name,team,token} closed
    return u


@app.get("/users/{name}")
def get_user(name: str) -> PublicUser:
    return lookup(name)


@app.get("/dicts/{name}")
def get_dict(name: str) -> PublicUser:
    d = {"name": name, "team": "core", "secret": 42}
    #@row d: present={name,team,secret} definite={name,team,secret} closed
    return d


@app.get("/plain/{name}")
def get_plain(name: str) -> dict:
    return {"name": name, "secret": 1}


if __name__ == "__main__":
    client = TestClient(app)
    for n in ("ann", "bob"):
        a = client.get(f"/users/{n}").json()
        #@row a: present={name,team} definite={name,team} absent={token} closed
        b = client.get(f"/dicts/{n}").json()
        #@row b: present={name,team} definite={name,team} absent={secret} closed
        c = client.get(f"/plain/{n}").json()
        #@row c: present={name,secret} definite={name,secret} closed
