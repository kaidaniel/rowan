# An Enum path parameter and a Literal query parameter: validation restricts them, so lookups keyed by their values never fail
from enum import Enum
from typing import Literal

from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()


class Model(str, Enum):
    alexnet = "alexnet"
    resnet = "resnet"


CONFIG = {"alexnet": {"layers": 8, "year": 2012}, "resnet": {"layers": 50, "year": 2015, "skip": True}}
SPEED = {"fast": 2, "slow": 1}


def describe(name, mode):
    #@value name: exactly={"alexnet","resnet"}
    #@value mode: exactly={"fast","slow"}
    cfg = CONFIG[name]                               #@ ok alexnet ; ok resnet
    #@row cfg: present={layers,year,skip} definite={layers,year} closed
    out = {"name": name, "layers": cfg["layers"], "speed": SPEED[mode]}  #@ ok layers ; ok fast ; ok slow
    return out


@app.get("/models/{model}")
def get_model(model: Model, mode: Literal["fast", "slow"] = "fast"):
    name = model.value
    #@value name: exactly={"alexnet","resnet"}
    info = describe(name, mode)
    if model is Model.resnet:
        info["skip"] = CONFIG[name]["skip"]          #@ ok skip
    #@row info: present={name,layers,speed,skip} definite={name,layers,speed} closed
    return info


if __name__ == "__main__":
    client = TestClient(app)
    assert client.get("/models/alexnet").json() == {"name": "alexnet", "layers": 8, "speed": 2}
    assert client.get("/models/resnet", params={"mode": "slow"}).json() == {"name": "resnet", "layers": 50, "speed": 1, "skip": True}
    assert client.get("/models/vgg").status_code == 422
    assert client.get("/models/resnet", params={"mode": "medium"}).status_code == 422
