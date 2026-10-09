# A batch of raw events aggregated with Counter / defaultdict / a literal-keyed tally; json round trip of a literal-keyed summary
import json
from collections import Counter, defaultdict
from typing import Any, Dict

from fastapi import FastAPI
from fastapi.testclient import TestClient

app = FastAPI()


def tally(events):
    status = {"ok": 0, "error": 0}
    kinds = Counter()
    by_user = defaultdict(list)
    for ev in events:
        status[ev["status"]] += 1                    #@ input status
        kinds[ev["kind"]] += 1                       #@ input kind
        by_user[ev.get("user", "anon")].append(ev["kind"])
    _ = 0  #@row status: present={ok,error} definite={ok,error} closed
    #@row kinds: open
    #@row by_user: open
    return status, kinds, by_user


def summary_of(status, kinds):
    s = {"ok": status["ok"], "error": status["error"], "top": kinds.most_common(1)[0][0] if kinds else None}  #@ ok ok ; ok error
    text = json.dumps(s)
    back = json.loads(text)
    #@row back: present={ok,error,top} definite={ok,error,top} closed
    return back


@app.post("/batch")
def batch(body: Dict[str, Any]):
    events = body["events"]                          #@ input events
    status, kinds, by_user = tally(events)
    out = summary_of(status, kinds)
    out["users"] = sorted(by_user)
    #@row out: present={ok,error,top,users} definite={ok,error,top,users} closed
    return out
#@schema param batch.body: always={events} only={events}
#@schema param batch.body['events']['*']: always={status,kind} only={status,kind}


if __name__ == "__main__":
    client = TestClient(app)
    good = {"events": [{"status": "ok", "kind": "a", "user": "u1"}, {"status": "error", "kind": "b"}, {"status": "ok", "kind": "a", "user": "u2"}]}
    assert client.post("/batch", json=good).json() == {"ok": 2, "error": 1, "top": "a", "users": ["anon", "u1", "u2"]}
    assert client.post("/batch", json={"events": []}).json()["top"] is None
    for bad in [{"events": [{"status": "weird", "kind": "a"}]}, {"evts": []}, {"events": [{"kind": "a"}]}]:
        try:
            client.post("/batch", json=bad)
        except KeyError:
            pass
