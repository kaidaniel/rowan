# Depends on a class (instance attributes set in __init__ from query params; Depends() shorthand) and on a callable instance
from fastapi import Depends, FastAPI
from fastapi.testclient import TestClient

app = FastAPI()


class Paging:
    def __init__(self, skip: int = 0, limit: int = 10):
        self.skip = skip
        self.limit = min(limit, 20)
        if limit > 20:
            self.capped = True
        _ = 0  #@row self: present={skip,limit,capped} definite={skip,limit} closed


class Contains:
    def __init__(self, word):
        self.word = word

    def __call__(self, q: str = ""):
        return self.word in q


has_bar = Contains("bar")
#@row has_bar: present={word} definite={word} closed


def window(p):
    w = {"from": p.skip, "to": p.skip + p.limit}
    if getattr(p, "capped", False):
        w["capped"] = True
    _ = 0  #@row w: present={from,to,capped} definite={from,to} closed
    return w


@app.get("/rows")
def rows(p: Paging = Depends(), found: bool = Depends(has_bar)):
    #@row p: present={skip,limit,capped} definite={skip,limit} closed
    w = window(p)
    w["found"] = found
    lim = p.limit                                    #@ ok limit
    c = p.capped                                     #@ possibly capped
    return w


if __name__ == "__main__":
    client = TestClient(app)
    for params in [{}, {"limit": 50, "q": "foobar"}, {"skip": 5, "q": "baz"}, {"limit": 100}]:
        try:
            client.get("/rows", params=params)
        except AttributeError:
            pass
