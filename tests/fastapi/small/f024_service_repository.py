# Request data flows handler -> service -> repository (in-memory dict); claims sit in the service and repository methods
from typing import Any, Dict

from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient


class Repo:
    def __init__(self):
        self.rows = {}
        self.next_id = 1

    def insert(self, rec):
        rid = self.next_id
        self.next_id += 1
        row = {"id": rid, **rec}
        #@row row: present={id,title,done,priority} definite={id,title,done} closed
        self.rows[rid] = row
        return row

    def update(self, rid, changes):
        row = self.rows[rid]
        row.update(changes)
        #@row row: present={id,title,done,priority} definite={id,title,done} closed
        return row


class TodoService:
    def __init__(self, repo):
        self.repo = repo

    def clean(self, data):
        rec = {"title": data["title"].strip(), "done": bool(data.get("done", False))}  #@ input title
        if "priority" in data:
            rec["priority"] = int(data["priority"])  #@ ok priority
        _ = 0  #@row rec: present={title,done,priority} definite={title,done} closed
        #@row data: definite={title} open
        return rec

    def create(self, data):
        return self.repo.insert(self.clean(data))

    def complete(self, rid):
        if rid not in self.repo.rows:
            raise HTTPException(status_code=404)
        return self.repo.update(rid, {"done": True})


repo = Repo()
service = TodoService(repo)
app = FastAPI()


@app.post("/todos")
def create_todo(payload: Dict[str, Any]):
    row = service.create(payload)
    #@row row: present={id,title,done,priority} definite={id,title,done} closed
    return row
#@schema param create_todo.payload: always={title} only={title}


@app.post("/todos/{rid}/done")
def finish(rid: int):
    row = service.complete(rid)
    d = row["done"]                                  #@ ok done
    return row


if __name__ == "__main__":
    client = TestClient(app)
    for p in [{"title": " a "}, {"title": "b", "done": True, "priority": "2"}, {"title": "c", "tag": "x"}, {"done": False}]:
        try:
            client.post("/todos", json=p)
        except KeyError:
            pass
    assert client.post("/todos/2/done").json()["done"] is True
    assert client.post("/todos/99/done").status_code == 404
    #@row repo: present={rows,next_id} definite={rows,next_id} closed
