# Header (underscore conversion, alias, Optional default) and Cookie parameters passed through a helper that builds a context dict
from typing import Optional

from fastapi import Cookie, FastAPI, Header
from fastapi.testclient import TestClient

app = FastAPI()


def build_context(agent, token, session, lang):
    ctx = {"agent": agent}
    if token:
        ctx["token"] = token
    if session is not None:
        ctx["session"] = session
    ctx["lang"] = (lang or "en").split(",")[0]
    #@row ctx: present={agent,token,session,lang} definite={agent,lang} closed
    return ctx


def describe(ctx):
    has_token = "token" in ctx
    #@value has_token: unknown
    who = ctx["agent"]                               #@ ok agent
    tok = ctx["token"]                               #@ possibly token
    return {"who": who, "token": tok}


@app.get("/whoami")
def whoami(
    user_agent: str = Header("none"),
    x_token: Optional[str] = Header(None),
    session_id: Optional[str] = Cookie(None),
    lang: Optional[str] = Header(None, alias="Accept-Language"),
):
    ctx = build_context(user_agent, x_token, session_id, lang)
    try:
        ctx.update(describe(ctx))
    except KeyError:
        ctx["anonymous"] = True
    _ = 0  #@row ctx: present={agent,token,session,lang,who,anonymous} definite={agent,lang} closed
    return ctx


if __name__ == "__main__":
    client = TestClient(app)
    a = client.get("/whoami").json()
    assert a["agent"] == "testclient" and a["lang"] == "en" and a["anonymous"]
    b = client.get("/whoami", headers={"X-Token": "t1", "Accept-Language": "de,en;q=0.5"}).json()
    assert b["token"] == "t1" and b["lang"] == "de" and b["who"] == "testclient"
    client.cookies.set("session_id", "s-42")
    c = client.get("/whoami", headers={"User-Agent": "curl"}).json()
    assert c["session"] == "s-42" and c["agent"] == "curl" and "token" not in c
    client.cookies.clear()
    d = client.get("/whoami", headers={"x-token": ""}).json()
    assert "token" not in d
