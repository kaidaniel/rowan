# Task tracker over a package: FastAPI handlers delegate to a store in another module, which fires hooks that rules in a third module registered by a decorator on import; pydantic models (field validator, IntEnum / str Enum, Optional date), JSON-mode dumps with exclude_none, dict bodies, a TestClient driver with literal and environment-given bodies
"""Hypothesis: the notes a handler returns are the merge of the callbacks a registry holds
(filled by decorators in another module at import time): with literal requests exactly the
keys of the callbacks' taken branches, for the framework's open requests the closed join of
all callbacks. A JSON-mode dump with exclude_none drops the None fields per request; the int
keys of a dict returned by a handler are strings after the JSON round trip."""
import json
import os

from fastapi.testclient import TestClient

from tasks import Priority, TaskIn
from tasks.api import STORE, app
from tasks.hooks import HOOKS, fire
from tasks.store import TODAY

if __name__ == "__main__":
    nh = len(HOOKS["created"])                   #@value nh: exactly={2}
    client = TestClient(app)

    r1 = client.post("/tasks", json={"title": "  write report ", "due": "2024-06-05"})
    s1 = r1.status_code                          #@value s1: exactly={201}
    b1 = r1.json()
    #@row b1: present={task,notes} definite={task,notes} absent={queue} closed
    t1 = b1["task"]
    #@row t1: present={title,priority,due,labels,id,status} definite={title,priority,due,labels,id,status} absent={closed_by} closed
    title = t1["title"]                          #@value title: exactly={"write report"}
    due1 = t1["due"]
    n1 = b1["notes"]                             #@row n1: present={due_in_days} definite={due_in_days} closed
    days = n1["due_in_days"]                     #@value days: exactly={4}

    r2 = client.post("/tasks", json={"title": "fix login", "priority": 3, "labels": ["bug"]}).json()
    #@row r2: present={task,notes,queue} definite={task,notes,queue} closed
    t2 = r2["task"]
    #@row t2: present={title,priority,labels,id,status} definite={title,priority,labels,id,status} absent={due,closed_by} closed
    n2 = r2["notes"]                             #@row n2: present={escalated,due_hint} definite={escalated,due_hint} closed
    pr = t2["priority"]
    s3 = client.post("/tasks", json={"title": "   "}).status_code   #@value s3: exactly={422}
    s4 = client.post("/tasks", json={"priority": 2}).status_code    #@value s4: exactly={422}
    r5 = client.post("/tasks", json={"title": "renew cert", "due": "2024-05-20", "labels": ["urgent"]}).json()
    n5 = r5["notes"]                             #@row n5: present={escalated,due_in_days} definite={escalated,due_in_days} closed
    late_by = n5["due_in_days"]                  #@value late_by: exactly={-12}
    id5 = r5["task"]["id"]

    c1 = client.post("/tasks/1/complete", json={"user": "kai"}).json()
    #@row c1: present={id,closed_by} definite={id,closed_by} absent={late} closed
    by1 = c1["closed_by"]                        #@value by1: exactly={kai}
    again = client.post("/tasks/1/complete", json={"user": "kai"}).status_code   #@value again: exactly={409}
    c3 = client.post("/tasks/3/complete", json={"user": "ops", "force": True, "reason": "rotation"}).json()
    #@row c3: present={id,closed_by,late} definite={id,closed_by,late} closed
    by3 = c3["closed_by"]                        #@value by3: exactly={"ops (rotation)"}
    try:
        client.post("/tasks/2/complete", json={"by": "x"})
    except KeyError:
        pass
    nf = client.post("/tasks/99/complete", json={"user": "a"}).status_code

    g1 = client.get("/tasks/1").json()
    #@row g1: present={title,priority,due,labels,id,status,closed_by} definite={title,priority,due,labels,id,status,closed_by} closed
    st1 = g1["status"]                           #@value st1: exactly={done}
    g404 = client.get("/tasks/42").status_code
    st = client.get("/stats").json()
    #@row st: present={total,by_status,events} definite={total,by_status,events} closed
    bys = st["by_status"]                        #@row bys: present={open,done} definite={open,done} closed
    ndone = bys["done"]                          #@value ndone: exactly={2}
    ev = st["events"]                            #@row ev: present={created,completed} definite={created,completed} closed

    imp = client.post("/tasks/import", json={"items": [{"title": "a"}, {"labels": []}, {"title": "b", "priority": 7}]}).json()
    #@row imp: present={created,errors} definite={created,errors} closed
    errs = imp["errors"]                         #@row errs: present={"1","2"} definite={"1","2"} closed
    e1 = tuple(errs["1"])                        #@value e1: exactly={("title",)}
    made = tuple(imp["created"])
    env_body = json.loads(os.environ.get("IMPORT_JSON", '{"items": [{"title": "env task", "due": "2024-07-01"}]}'))
    #@schema json.loads@L73: always={items} only={items}
    imp2 = client.post("/tasks/import", json=env_body).json()
    #@row imp2: present={created,errors} definite={created,errors} closed

    task, notes = STORE.create(TaskIn(title="direct", priority=Priority.LOW))
    #@row notes: present={due_hint} definite={due_hint} closed
    done = fire("completed", task, {"today": TODAY, "user": "me"})
    #@row done: present={closed_by} definite={closed_by} absent={late} closed
