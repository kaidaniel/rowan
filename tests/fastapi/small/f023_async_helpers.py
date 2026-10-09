# async handlers awaiting async helpers (two layers) next to a sync handler calling the same plain helper
import asyncio

from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()

PROFILES = {"ann": {"name": "ann", "langs": ["py", "rs"]}, "bob": {"name": "bob", "langs": [], "bio": "hi"}}


async def fetch_profile(name):
    await asyncio.sleep(0)
    prof = PROFILES.get(name, {"name": name, "langs": []})
    #@row prof: present={name,langs,bio} definite={name,langs} closed
    return dict(prof)


def score(prof):
    s = len(prof["langs"]) * 10                      #@ ok langs
    if "bio" in prof:
        s += 1
    return s


async def enrich(name):
    prof = await fetch_profile(name)
    prof["score"] = score(prof)
    #@row prof: present={name,langs,bio,score} definite={name,langs,score} closed
    return prof


@app.get("/a/{name}")
async def a_profile(name: str):
    prof = await enrich(name)
    out = {"name": prof["name"], "score": prof["score"]}  #@ ok name ; ok score
    #@row out: present={name,score} definite={name,score} closed
    return out


@app.get("/s/{name}")
def s_profile(name: str):
    prof = PROFILES.get(name, {"name": name, "langs": []})
    return {"score": score(prof), "bio": prof["bio"]}  #@ possibly bio


if __name__ == "__main__":
    client = TestClient(app)
    assert client.get("/a/ann").json() == {"name": "ann", "score": 20}
    assert client.get("/a/bob").json() == {"name": "bob", "score": 1}
    assert client.get("/a/zed").json() == {"name": "zed", "score": 0}
    for n in ("ann", "bob"):
        try:
            client.get(f"/s/{n}")
        except KeyError:
            pass
