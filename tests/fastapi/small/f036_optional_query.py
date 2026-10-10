"""Optional query parameters (`Optional[str]`, `int | None`, `Union[float, None]`) convert the
text the client sends like the inner scalar; a float query is parsed ("5" is 5.0); an absent one
is the default; text that is not a float is a 422."""
from typing import Optional, Union

from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()


@app.get("/q")
def q(tag: Optional[str] = None, n: int | None = None, x: Union[float, None] = None, lo: float = 0.0):
    return {"tag": tag, "n": n, "x": x, "lo": lo}


if __name__ == "__main__":
    c = TestClient(app)
    r = c.get("/q", params={"tag": "office", "n": "3", "x": "2.5", "lo": "5"}).json()
    tag = r["tag"]                               #@value tag: exactly={"office"}
    n = r["n"]                                   #@value n: exactly={3}
    x = r["x"]                                   #@value x: exactly={2.5}
    lo = r["lo"]                                 #@value lo: exactly={5.0}
    d = c.get("/q").json()
    dtag = d["tag"]                              #@value dtag: exactly={None}
    dlo = d["lo"]                                #@value dlo: exactly={0.0}
    bad = c.get("/q", params={"lo": "abc"}).status_code   #@value bad: exactly={422}
