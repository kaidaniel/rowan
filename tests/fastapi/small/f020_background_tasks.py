# BackgroundTasks.add_task(f, *args, **kwargs): the task function runs after the response with the arguments given here
from typing import Any, Dict

from fastapi import BackgroundTasks, FastAPI
from fastapi.testclient import TestClient

app = FastAPI()

OUTBOX = []
AUDIT = []


def send_email(to, ctx):
    #@row ctx: present={subject,name,plan} definite={subject,name} closed
    msg = {"to": to, "subject": ctx["subject"], "text": f"hi {ctx['name']}"}  #@ ok subject ; ok name
    if "plan" in ctx:
        msg["plan"] = ctx["plan"]
    #@row msg: present={to,subject,text,plan} definite={to,subject,text} closed
    OUTBOX.append(msg)


def audit(action, **fields):
    entry = {"action": action, **fields}
    #@row fields: present={user,plan} definite={user} closed
    AUDIT.append(entry)


@app.post("/signup", status_code=201)
def signup(body: Dict[str, Any], tasks: BackgroundTasks):
    email = body["email"]                            #@ input email
    ctx = {"subject": "welcome", "name": body.get("name", "friend")}
    if body.get("plan"):
        ctx["plan"] = body["plan"]                   #@ ok plan
        tasks.add_task(audit, "upgrade", user=email, plan=ctx["plan"])
    tasks.add_task(send_email, email, ctx)
    tasks.add_task(audit, "signup", user=email)
    return {"queued": True}
#@schema param signup.body: always={email} only={email}


if __name__ == "__main__":
    client = TestClient(app)
    for body in [{"email": "a@x"}, {"email": "b@x", "name": "Bob", "plan": "pro"}, {"email": "c@x", "plan": ""}, {"name": "nobody"}]:
        try:
            r = client.post("/signup", json=body)
            assert r.status_code == 201
        except KeyError:
            pass
    assert [m["to"] for m in OUTBOX] == ["a@x", "b@x", "c@x"]
    assert [a["action"] for a in AUDIT] == ["signup", "upgrade", "signup", "signup"]
    m = OUTBOX[1]
    #@row m: present={to,subject,text,plan} definite={to,subject,text,plan} closed
