"""A pydantic ValidationError raised by the handler's own code is no request error: the
TestClient re-raises it at the call (no 422 response); a model built from len() values
validates (len is an int); a request body that breaks the model is a 422."""
from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel, ValidationError

app = FastAPI()


class Summary(BaseModel):
    name: str
    n: int


class Req(BaseModel):
    items: list[str] = []


@app.post("/count/{name}")
def count(name: str, req: Req):
    return Summary(name=name, n=len(req.items))


@app.get("/broken")
def broken():
    return Summary(name="x", n="many")


client = TestClient(app)
r = client.post("/count/a", json={"items": ["p", "q"]})
code = r.status_code                              #@value code: exactly={200}
body = r.json()
#@row body: present={name,n} definite={name,n} closed
n = body["n"]                                     #@value n: exactly={2}
bad = client.post("/count/a", json={"items": "nope"}).status_code   #@value bad: exactly={422}
outcome = "response"
try:
    client.get("/broken")
except ValidationError:
    outcome = "raised"
out = outcome                                     #@value out: exactly={"raised"}
