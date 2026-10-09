# HTTPException raised from helpers two levels down: status codes, a dict detail (the response is {"detail": detail}), headers
from typing import Any, Dict

from fastapi import FastAPI, HTTPException
from fastapi.testclient import TestClient

app = FastAPI()

ACCOUNTS = {"a1": {"balance": 100, "owner": "ann"}, "a2": {"balance": 5, "owner": "bob", "frozen": True}}


def require_field(body, field):
    if field not in body:
        raise HTTPException(status_code=400, detail={"error": "missing", "field": field})
    return body[field]                               #@ ok amount ; ok account


def get_account(acc_id):
    acc = ACCOUNTS.get(acc_id)
    if acc is None:
        raise HTTPException(status_code=404, detail={"error": "no account", "field": "account"})
    if acc.get("frozen"):
        raise HTTPException(status_code=423, detail={"error": "frozen", "field": "account"}, headers={"Retry-After": "60"})
    #@row acc: present={balance,owner,frozen} definite={balance,owner} closed
    return acc


def withdraw(acc, amount):
    if amount > acc["balance"]:                      #@ ok balance
        raise HTTPException(status_code=409, detail={"error": "insufficient", "field": "amount", "have": acc["balance"]})
    acc["balance"] -= amount
    return acc["balance"]


@app.post("/withdraw")
def do_withdraw(body: Dict[str, Any]):
    acc = get_account(require_field(body, "account"))
    left = withdraw(acc, require_field(body, "amount"))
    #@row body: definite={account,amount} open
    return {"left": left, "owner": acc["owner"]}     #@ ok owner


if __name__ == "__main__":
    client = TestClient(app)
    codes = []
    for body in [{"account": "a1", "amount": 10}, {"account": "a1"}, {"amount": 1}, {"account": "zz", "amount": 1},
                 {"account": "a2", "amount": 1}, {"account": "a1", "amount": 1000}]:
        r = client.post("/withdraw", json=body)
        codes.append(r.status_code)
        if r.status_code != 200:
            det = r.json()["detail"]
            #@row det: present={error,field,have} definite={error,field} closed
    assert codes == [200, 400, 400, 404, 423, 409], codes
