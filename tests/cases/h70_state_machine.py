"""An order state machine: a nested transition table (state -> event -> (next, handler)); the
handlers mutate the order object and its metadata; the order is passed through `run`/`apply`.

Expectations were written from the program text before running the engine."""
import sys


class Order:
    def __init__(self, oid, items):
        self.id = oid
        self.items = items
        self.state = "new"
        self.history = []
        self.meta = {}


def on_pay(order, ev):
    order.paid = {"amount": ev["amount"], "method": ev.get("method", "card")}
    order.meta["paid_at"] = ev["at"]


def on_ship(order, ev):
    order.shipment = {"carrier": ev["carrier"]}
    order.meta["shipped_at"] = ev["at"]


def on_cancel(order, ev):
    order.meta["reason"] = ev.get("reason", "unknown")
    if "refund" in ev:
        order.refund = {"amount": ev["refund"]}


TRANSITIONS = {
    "new": {"pay": ("paid", on_pay), "cancel": ("cancelled", on_cancel)},
    "paid": {"ship": ("shipped", on_ship), "cancel": ("cancelled", on_cancel)},
}


def apply(order, ev):
    nxt, handler = TRANSITIONS[order.state][ev["name"]]
    handler(order, ev)
    order.history.append({"from": order.state, "to": nxt, "event": ev["name"]})
    order.state = nxt
    return order


def run(order, events):
    for ev in events:
        apply(order, ev)
    return order


o1 = run(Order(1, [{"sku": "a", "qty": 1}]),
         [{"name": "pay", "amount": 5, "at": 1}, {"name": "ship", "carrier": "dhl", "at": 2}])
_ = 0                                     #@row o1: present={id,items,state,history,meta,paid,shipment} definite={id,items,state,history,meta,paid,shipment} closed
paid = o1.paid                            #@row paid: present={amount,method} definite={amount,method} closed
meta1 = o1.meta                           #@row meta1: present={paid_at,shipped_at} definite={paid_at,shipped_at} closed
h = o1.history[1]                         #@row h: present={from,to,event} definite={from,to,event} closed
item = o1.items[0]                        #@row item: present={sku,qty} definite={sku,qty} closed

o2 = run(Order(2, []), [{"name": "pay", "amount": 1, "at": 1, "method": "cash"},
                        {"name": "cancel", "refund": 1, "at": 3}])
_ = 0                                     #@row o2: present={id,items,state,history,meta,paid,refund} definite={id,items,state,history,meta,paid,refund} closed
meta2 = o2.meta                           #@row meta2: present={paid_at,reason} definite={paid_at,reason} closed
refund = o2.refund                        #@row refund: present={amount} definite={amount} closed

# Cancel without a refund key: the refund attribute is never set.
o3 = Order(3, [])
apply(o3, {"name": "cancel", "at": 1})
_ = 0                                     #@row o3: present={id,items,state,history,meta} definite={id,items,state,history,meta} closed
meta3 = o3.meta                           #@row meta3: present={reason} definite={reason} closed

# Event name from the command line: from "new" it is either pay or cancel.
o4 = Order(4, [])
apply(o4, {"name": sys.argv[1], "amount": 1, "at": 1, "carrier": "x"})
_ = 0                                     #@row o4: present={id,items,state,history,meta,paid} definite={id,items,state,history,meta} closed
meta4 = o4.meta                           #@row meta4: present={paid_at,reason} definite={} closed
o4.paid["amount"]                         #@ ok amount
o1.meta["reason"]                         #@ missing reason
