"""Order intake: a customer registry, carts turned into order records, validation against
stock, priorities, and a repository keyed by functions with secondary indexes."""
from collections import defaultdict

from .models import order_record


class Repo:
    def __init__(self, name, key_fn, index_fns=None):
        self.name = name
        self.key_fn = key_fn
        self.index_fns = dict(index_fns or {})
        self.items = {}
        self.indexes = {iname: defaultdict(list) for iname in self.index_fns}
        self.events = []

    def add(self, obj):
        k = self.key_fn(obj)
        self.items[k] = obj
        for iname, fn in self.index_fns.items():
            self.indexes[iname][fn(obj)].append(k)
        self.events.append({"op": "add", "key": k})
        return obj

    def get(self, k):
        return self.items[k]

    def find(self, iname, value):
        return [self.items[k] for k in self.indexes[iname].get(value, [])]

    def where(self, pred):
        return [o for o in self.items.values() if pred(o)]

    def first(self, pred, default=None):
        for o in self.items.values():
            if pred(o):
                return o
        return default

    def count(self):
        return len(self.items)

    def describe(self):
        return {"name": self.name, "n": self.count(), "indexes": sorted(self.indexes)}


class Customer:
    def __init__(self, cid, name, tier, address):
        self.cid = cid
        self.name = name
        self.tier = tier
        self.address = address
        self.orders = []

    def is_vip(self):
        return self.tier == "gold"

    def to_dict(self):
        d = {"id": self.cid, "name": self.name, "tier": self.tier, "address": dict(self.address)}
        if self.orders:
            d["orders"] = list(self.orders)
        return d


def seed_customers(repo):
    repo.add(Customer(1, "Ann", "gold", {"city": "Berlin", "zone": "A", "zip": "10115"}))
    repo.add(Customer(2, "Bob", "basic", {"city": "Paris", "zone": "B", "zip": "75001"}))
    repo.add(Customer(3, "Cy", "silver", {"city": "Oslo", "zone": "C", "zip": ""}))
    return repo


class Cart:
    def __init__(self, customer):
        self.customer = customer
        self.lines = []
        self.notes = {}

    def add(self, sku, qty):
        for line in self.lines:
            if line["sku"] == sku:
                line["qty"] += qty
                return self
        self.lines.append({"sku": sku, "qty": qty})
        return self

    def note(self, key, value):
        self.notes[key] = value
        return self

    def is_empty(self):
        return not self.lines


class OrderBook:
    def __init__(self, inventory, customers):
        self.inventory = inventory
        self.customers = customers
        self.orders = Repo("orders", lambda o: o["id"], {"customer": lambda o: o["customer"]["id"], "status": lambda o: o["status"]})
        self.seq = 100
        self.rejected = []

    def validate(self, cart):
        problems = []
        if cart.is_empty():
            problems.append({"why": "empty"})
        for line in cart.lines:
            if line["sku"] not in self.inventory.products:
                problems.append({"why": "unknown", "sku": line["sku"]})
            elif self.inventory.available(line["sku"]) < line["qty"]:
                problems.append({"why": "stock", "sku": line["sku"], "available": self.inventory.available(line["sku"])})
        return problems

    def place(self, cart, priority="normal"):
        problems = self.validate(cart)
        if problems:
            rec = {"customer": cart.customer.cid, "problems": problems}
            self.rejected.append(rec)
            return None
        self.seq += 1
        order = order_record(self.seq, cart.customer.to_dict(), cart.lines, priority)
        if cart.notes:
            order["notes"] = dict(cart.notes)
        for line in cart.lines:
            self.inventory.reserve(line["sku"], line["qty"])
        self.orders.add(order)
        cart.customer.orders.append(order["id"])
        return order

    def set_status(self, oid, status):
        order = self.orders.get(oid)
        prev = order["status"]
        order["status"] = status
        order.setdefault("transitions", []).append({"from": prev, "to": status})
        return order

    def open_orders(self):
        return self.orders.where(lambda o: o["status"] == "open")

    def by_customer(self, cid):
        return self.orders.find("customer", cid)

    def totals(self):
        out = {"orders": self.orders.count(), "rejected": len(self.rejected), "lines": 0}
        for o in self.orders.items.values():
            out["lines"] += len(o["lines"])
        return out
