# A yield dependency: the handler gets the yielded session dict; the code after yield runs once the request is done
from fastapi import Depends, FastAPI
from fastapi.testclient import TestClient

app = FastAPI()

CLOSED = []
STORE = {}


def session():
    s = {"open": True, "ops": []}
    try:
        yield s
    finally:
        s["open"] = False
        s["n_ops"] = len(s["ops"])                   #@ ok ops
        #@row s: present={open,ops,n_ops} definite={open,ops,n_ops} closed
        CLOSED.append(s)


def put(sess, key, value):
    sess["ops"].append(("put", key))                 #@ ok ops
    STORE[key] = value


@app.post("/kv/{key}")
def write(key: str, value: int = 0, sess: dict = Depends(session)):
    _ = 0  #@row sess: present={open,ops} definite={open,ops} closed
    put(sess, key, value)
    if value > 10:
        put(sess, key + "_big", value)
    return {"ops": len(sess["ops"]), "open": sess["open"]}


if __name__ == "__main__":
    client = TestClient(app)
    assert client.post("/kv/a", params={"value": 3}).json() == {"ops": 1, "open": True}
    assert client.post("/kv/b", params={"value": 30}).json() == {"ops": 2, "open": True}
    assert client.post("/kv/c").json() == {"ops": 1, "open": True}
    last = CLOSED[-1]
    #@row last: present={open,ops,n_ops} definite={open,ops,n_ops} closed
    still = last["open"]                             #@value still: exactly={False}
    n = len(CLOSED)                                  #@value n: exactly={3}
    keys = sorted(STORE)
    #@row STORE: present={a,b,b_big,c} definite={a,b,b_big,c} closed
