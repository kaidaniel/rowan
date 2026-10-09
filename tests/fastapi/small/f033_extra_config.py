# Body models with extra="allow" (unknown keys become attributes: an open record) and extra="forbid" (unknown keys: 422)
from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import BaseModel, ConfigDict

app = FastAPI()


class Event(BaseModel):
    model_config = ConfigDict(extra="allow")
    kind: str
    ts: int = 0


class Strict(BaseModel):
    model_config = ConfigDict(extra="forbid")
    name: str
    size: int = 1


def flatten(ev):
    d = ev.model_dump()
    #@row d: present={kind,ts} definite={kind,ts} open
    extras = ev.model_extra
    #@row extras: open
    d["n_extra"] = len(extras)
    return d


@app.post("/events")
def ingest(ev: Event):
    d = flatten(ev)
    k = d["kind"]                                    #@ ok kind
    src = d.get("source", "unknown")
    return {"kind": k, "source": src, "n_extra": d["n_extra"]}  #@ ok n_extra


@app.post("/strict")
def strict(s: Strict):
    d = s.model_dump()
    #@row d: present={name,size} definite={name,size} closed
    s.colour                                         #@ missing colour
    return d


if __name__ == "__main__":
    client = TestClient(app)
    assert client.post("/events", json={"kind": "click"}).json() == {"kind": "click", "source": "unknown", "n_extra": 0}
    assert client.post("/events", json={"kind": "view", "source": "web", "page": "/"}).json() == {"kind": "view", "source": "web", "n_extra": 2}
    assert client.post("/events", json={"ts": 3}).status_code == 422
    assert client.post("/strict", json={"name": "a", "colour": "red"}).status_code == 422
    for body in [{"name": "a"}, {"name": "b", "size": 4}]:
        try:
            client.post("/strict", json=body)
        except AttributeError:
            pass
