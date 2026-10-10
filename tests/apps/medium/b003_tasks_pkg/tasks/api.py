"""HTTP routes over one store."""
from fastapi import Body, FastAPI, HTTPException
from pydantic import ValidationError

import tasks.rules  # noqa: F401
from tasks.models import TaskIn
from tasks.store import TaskStore

app = FastAPI()
STORE = TaskStore()


@app.post("/tasks", status_code=201)
def create_task(data: TaskIn):
    task, notes = STORE.create(data)
    body = {"task": task.public(), "notes": notes}
    if notes.get("escalated"):
        body["queue"] = "urgent"
    _ = 0  #@row body: present={task,notes,queue} definite={task,notes} closed
    return body


@app.get("/tasks/{tid}")
def get_task(tid: int):
    task = STORE.tasks.get(tid)
    if task is None:
        raise HTTPException(status_code=404, detail="no such task")
    return task.public()


@app.post("/tasks/{tid}/complete")
def complete_task(tid: int, body: dict = Body(...)):
    if tid not in STORE.tasks:
        raise HTTPException(status_code=404, detail="no such task")
    user = body["user"]                          #@ possibly user
    if body.get("force"):
        user = f"{user} ({body['reason']})"
    notes = STORE.complete(tid, user)
    if notes is None:
        raise HTTPException(status_code=409, detail="already done")
    out = {"id": tid, **notes}
    #@row out: present={id,closed_by,late} definite={id,closed_by} closed
    return out
#@schema param complete_task.body: always={user} sometimes={reason} only={user,reason}


@app.get("/stats")
def stats():
    return STORE.stats()


@app.post("/tasks/import")
def import_tasks(payload: dict = Body(...)):
    created, errors = [], {}
    for i, raw in enumerate(payload["items"]):
        try:
            task, _ = STORE.create(TaskIn.model_validate(raw))
            created.append(task.id)
        except ValidationError as e:
            errors[i] = [err["loc"][0] for err in e.errors()]
    return {"created": created, "errors": errors}
#@schema param import_tasks.payload: always={items} only={items}
#@schema param import_tasks.payload['items']['*']: always={title} only={title}
