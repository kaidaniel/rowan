# Event dispatch through a registry filled by a decorator factory (functools.wraps wrappers that add a key to the handler's result), a Counter of calls, events from literals and from JSON in os.environ
"""Hypothesis: the dispatcher's result depends on which registered wrapper runs. With a literal
event type exactly one handler runs (its keys, plus the "kind" the wrapper adds, exactly); with
an event from input every registered handler and the fallback may run, so the result is the
closed join of their rows with only "kind" definite. The wrapper's mutation of the handler's
fresh dict ("kind") must reach every call site, and the Counter the wrapper bumps is a module
global changed through a closure."""
import functools
import json
import os
from collections import Counter

HANDLERS = {}
CALLS = Counter()


def handles(kind):
    def deco(fn):
        @functools.wraps(fn)
        def wrapper(event):
            CALLS[kind] += 1
            out = fn(event)
            out["kind"] = kind
            return out
        HANDLERS[kind] = wrapper
        return wrapper
    return deco


@handles("signup")
def on_signup(event):
    user = event["user"]
    return {"user": user.lower(), "plan": event.get("plan", "free")}


@handles("purchase")
def on_purchase(event):
    total = sum(item["price"] * item.get("qty", 1) for item in event["items"])
    out = {"user": event["user"], "total": total}
    if total >= 100:
        out["vip"] = True
    return out


@handles("refund")
def on_refund(event):
    order = event["order"]                       #@ ok order ; input order
    return {"order": order, "amount": -abs(event["amount"])}


def dispatch(event):
    handler = HANDLERS.get(event.get("type"))
    if handler is None:
        return {"kind": "unknown", "raw": sorted(event)}
    res = handler(event)
    #@row res: present={user,plan,total,vip,order,amount,kind} definite={kind} closed
    return res


def user_of(res):
    return res["user"]                           #@ possibly user


def summarize(events):
    by_kind = Counter()
    users = set()
    for ev in events:
        res = dispatch(ev)
        by_kind[res["kind"]] += 1                #@ ok kind
        if "user" in res:
            users.add(res["user"])
    return {"counts": dict(by_kind), "users": sorted(users)}


if __name__ == "__main__":
    s = dispatch({"type": "signup", "user": "Ann"})
    #@row s: present={user,plan,kind} definite={user,plan,kind} closed
    plan = s["plan"]                             #@value plan: exactly={free}
    who = s["user"]                              #@value who: exactly={ann}
    p = dispatch({"type": "purchase", "user": "bo", "items": [{"price": 60}, {"price": 25, "qty": 2}]})
    #@row p: present={user,total,vip,kind} definite={user,total,vip,kind} closed
    total = p["total"]                           #@value total: exactly={110}
    p2 = dispatch({"type": "purchase", "user": "cy", "items": [{"price": 5}]})
    #@row p2: present={user,total,kind} definite={user,total,kind} absent={vip} closed
    k2 = p2["kind"]                              #@value k2: exactly={purchase}
    rf = dispatch({"type": "refund", "order": "o9", "amount": 2.5})
    #@row rf: present={order,amount,kind} definite={order,amount,kind} closed
    amt = rf["amount"]                           #@value amt: exactly={-2.5}
    u = dispatch({"type": "audit", "who": "x", "at": 3})
    #@row u: present={kind,raw} definite={kind,raw} closed
    raw = tuple(u["raw"])                        #@value raw: exactly={("at","type","who")}
    name = on_refund.__name__                    #@value name: exactly={on_refund}
    n_purchase = CALLS["purchase"]               #@value n_purchase: exactly={2}
    for res in (s, u):
        try:
            user_of(res)
        except KeyError:
            pass

    summ = summarize([{"type": "signup", "user": "Dee", "plan": "pro"}, {"type": "purchase", "user": "ann", "items": []}, {"type": "nope"}])
    #@row summ: present={counts,users} definite={counts,users} closed
    counts = summ["counts"]
    #@row counts: present={signup,purchase,unknown} definite={signup,purchase,unknown} closed
    users = tuple(summ["users"])                 #@value users: exactly={("ann","dee")}
    n_signup = CALLS["signup"]                   #@value n_signup: exactly={2}

    incoming = json.loads(os.environ.get("EVENTS_JSON", '[{"type": "refund", "order": "o1", "amount": 4}]'))
    #@schema json.loads@L107['*']: sometimes={user,items,order,amount} only={user,items,order,amount}
    r = dispatch(incoming[0])
    #@row r: present={user,plan,total,vip,order,amount,kind,raw} definite={kind} closed
    summ2 = summarize(incoming)
    counts2 = summ2["counts"]
    #@row counts2: present={signup,purchase,refund,unknown} definite={} closed
