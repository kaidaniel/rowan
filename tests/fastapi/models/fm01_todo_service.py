# Todo service: routers with prefixes, a repository and a service layer, a yield unit-of-work dependency, pydantic create / patch models, raw dict imports, filters from query params, response models
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, Depends, FastAPI, Header, HTTPException, Query
from fastapi.testclient import TestClient
from pydantic import BaseModel, Field


# ---------------------------------------------------------------- models

class TodoIn(BaseModel):
    title: str = Field(min_length=1)
    priority: int = 3
    tags: List[str] = []
    due: Optional[str] = None


class TodoPatch(BaseModel):
    title: Optional[str] = None
    priority: Optional[int] = None
    done: Optional[bool] = None


class TodoOut(BaseModel):
    id: int
    title: str
    priority: int
    done: bool
    tags: List[str]
    due: Optional[str] = None


# ---------------------------------------------------------------- repository

class TodoRepo:
    def __init__(self):
        self.rows = {}
        self.seq = 0
        self.journal = []

    def add(self, data):
        self.seq += 1
        row = {"id": self.seq, "done": False, **data}
        #@row row: present={id,done,title,priority,tags,due,owner} definite={id,done,title,priority,tags,due,owner} closed
        self.rows[self.seq] = row
        return row

    def get(self, tid):
        row = self.rows.get(tid)
        if row is None:
            raise HTTPException(status_code=404, detail={"error": "not_found", "id": tid})
        return row

    def update(self, tid, changes):
        row = self.get(tid)
        row.update(changes)
        #@row row: present={id,done,title,priority,tags,due,owner} definite={id,done,title,priority,tags,due,owner} closed
        return row

    def delete(self, tid):
        row = self.get(tid)
        del self.rows[tid]
        return row


REPO = TodoRepo()


# ---------------------------------------------------------------- unit of work (yield dependency)

def unit_of_work():
    uow = {"ops": [], "committed": False}
    try:
        yield uow
        uow["committed"] = True
    finally:
        entry = {"n_ops": len(uow["ops"]), "committed": uow["committed"]}  #@ ok ops ; ok committed
        #@row entry: present={n_ops,committed} definite={n_ops,committed} closed
        REPO.journal.append(entry)


def current_owner(x_owner: str = Header("anonymous")):
    return x_owner.strip().lower() or "anonymous"


# ---------------------------------------------------------------- service

class TodoService:
    def __init__(self, repo, uow, owner):
        self.repo = repo
        self.uow = uow
        self.owner = owner

    def create(self, todo):
        _ = 0  #@row todo: present={title,priority,tags,due} definite={title,priority,tags,due} closed
        data = todo.model_dump()
        data["owner"] = self.owner
        data["tags"] = sorted(set(t.lower() for t in data["tags"]))  #@ ok tags
        self.uow["ops"].append("create")
        return self.repo.add(data)

    def import_raw(self, raw):
        title = raw["title"]                         #@ input title
        prio = raw.get("prio", raw.get("priority", 3))
        data = {"title": str(title), "priority": int(prio), "tags": list(raw.get("tags", [])), "due": raw.get("due"), "owner": self.owner}
        #@row data: present={title,priority,tags,due,owner} definite={title,priority,tags,due,owner} closed
        #@row raw: definite={title} open
        self.uow["ops"].append("import")
        return self.repo.add(data)

    def patch(self, tid, patch):
        changes = patch.model_dump(exclude_unset=True)
        #@row changes: present={title,priority,done} closed
        if "title" in changes and not changes["title"]:  #@ ok title
            raise HTTPException(status_code=422, detail="empty title")
        row = self.repo.get(tid)
        if row["owner"] != self.owner:               #@ ok owner
            raise HTTPException(status_code=403, detail="not yours")
        self.uow["ops"].append("patch")
        return self.repo.update(tid, changes)

    def listing(self, flt):
        rows = [r for r in self.repo.rows.values() if r["owner"] == self.owner]
        if flt["done"] is not None:                  #@ ok done
            rows = [r for r in rows if r["done"] == flt["done"]]
        if flt["tag"]:                               #@ ok tag
            rows = [r for r in rows if flt["tag"] in r["tags"]]
        rows.sort(key=lambda r: (r["priority"], r["id"]))
        return rows[: flt["limit"]]                  #@ ok limit


def get_service(uow: dict = Depends(unit_of_work), owner: str = Depends(current_owner)):
    _ = 0  #@row uow: present={ops,committed} definite={ops,committed} closed
    return TodoService(REPO, uow, owner)


def list_filters(done: Optional[bool] = None, tag: Optional[str] = None, limit: int = Query(20, ge=1, le=100)):
    flt = {"done": done, "tag": tag, "limit": limit}
    #@row flt: present={done,tag,limit} definite={done,tag,limit} closed
    return flt


# ---------------------------------------------------------------- routes

todos = APIRouter(prefix="/todos", tags=["todos"])


@todos.post("", response_model=TodoOut, status_code=201)
def create_todo(todo: TodoIn, svc: TodoService = Depends(get_service)):
    row = svc.create(todo)
    #@row row: present={id,done,title,priority,tags,due,owner} definite={id,done,title,priority,tags,due,owner} closed
    return row


@todos.post("/import")
def import_todos(body: Dict[str, Any], svc: TodoService = Depends(get_service)):
    items = body["items"]                            #@ input items
    created = [svc.import_raw(raw)["id"] for raw in items]
    return {"created": created, "source": body.get("source", "api")}
#@schema param import_todos.body: always={items} only={items}
#@schema param import_todos.body['items']['*']: always={title} only={title}


@todos.get("", response_model=List[TodoOut])
def list_todos(flt: dict = Depends(list_filters), svc: TodoService = Depends(get_service)):
    return svc.listing(flt)


@todos.patch("/{tid}", response_model=TodoOut, response_model_exclude_none=True)
def patch_todo(tid: int, patch: TodoPatch, svc: TodoService = Depends(get_service)):
    row = svc.patch(tid, patch)
    t = row["title"]                                 #@ ok title
    return row


@todos.delete("/{tid}")
def delete_todo(tid: int, svc: TodoService = Depends(get_service)):
    row = svc.repo.delete(tid)
    svc.uow["ops"].append("delete")
    return {"deleted": row["id"], "title": row["title"]}  #@ ok id ; ok title


app = FastAPI()
app.include_router(todos, prefix="/api")


@app.get("/health")
def health():
    stats = {"rows": len(REPO.rows), "journal": len(REPO.journal)}
    #@row stats: present={rows,journal} definite={rows,journal} closed
    return stats


if __name__ == "__main__":
    client = TestClient(app)
    ann = {"X-Owner": "Ann"}
    r = client.post("/api/todos", json={"title": "write spec", "tags": ["Work", "work"]}, headers=ann)
    assert r.status_code == 201, r.text
    created = r.json()
    #@row created: present={id,title,priority,done,tags,due} definite={id,title,priority,done,tags,due} absent={owner} closed
    client.post("/api/todos", json={"title": "buy milk", "priority": 1, "due": "today", "colour": "x"}, headers=ann)
    client.post("/api/todos", json={"title": "bob's", "priority": 2})
    assert client.post("/api/todos", json={"title": ""}).status_code == 422
    assert client.post("/api/todos", json={"priority": 1}).status_code == 422

    imp = client.post("/api/todos/import", json={"items": [{"title": "a", "prio": 5}, {"title": "b", "tags": ["x"], "extra": 1}], "source": "csv"}, headers=ann)
    assert imp.json() == {"created": [4, 5], "source": "csv"}, imp.json()
    for bad in [{"items": [{"name": "no title"}]}, {"todos": []}]:
        try:
            client.post("/api/todos/import", json=bad, headers=ann)
        except KeyError:
            pass

    listing = client.get("/api/todos", headers=ann).json()
    assert [t["title"] for t in listing] == ["buy milk", "write spec", "b", "a"], listing
    first = listing[0]
    #@row first: present={id,title,priority,done,tags,due} definite={id,title,priority,done,tags,due} closed
    assert [t["id"] for t in client.get("/api/todos", params={"tag": "x"}, headers=ann).json()] == [5]
    assert client.get("/api/todos", params={"limit": 0}).status_code == 422
    assert client.get("/api/todos").json()[0]["title"] == "bob's"

    p = client.patch("/api/todos/1", json={"done": True}, headers=ann).json()  # (row had due as maybe: todo 1's due is None, excluded)
    #@row p: present={id,title,priority,done,tags} definite={id,title,priority,done,tags} closed
    assert p["done"] is True and "due" not in p
    assert client.patch("/api/todos/2", json={"title": "milk!"}, headers=ann).json()["due"] == "today"
    assert client.patch("/api/todos/3", json={"done": True}, headers=ann).status_code == 403
    assert client.patch("/api/todos/1", json={"title": ""}, headers=ann).status_code == 422
    assert client.patch("/api/todos/99", json={}, headers=ann).status_code == 404

    assert client.delete("/api/todos/4").json() == {"deleted": 4, "title": "a"}
    assert client.delete("/api/todos/4").status_code == 404
    h = client.get("/health").json()
    #@row h: present={rows,journal} definite={rows,journal} closed
    last = REPO.journal[-1]
    #@row last: present={n_ops,committed} definite={n_ops,committed} closed
    committed = [j["committed"] for j in REPO.journal]
    assert False in committed and True in committed
