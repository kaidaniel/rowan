"""In-memory storage: stock with reservations, orders keyed by id, per-customer counts."""
from collections import Counter


class Inventory:
    def __init__(self, stock):
        self.stock = dict(stock)
        self.reserved = Counter()

    def available(self, sku):
        return self.stock.get(sku, 0) - self.reserved[sku]

    def reserve(self, items):
        short = {}
        for it in items:
            free = self.available(it.sku)
            if free < it.qty:
                short[it.sku] = it.qty - max(free, 0)
        if short:
            return short
        for it in items:
            self.reserved[it.sku] += it.qty
        return {}

    def release(self, lines):
        for sku, qty in lines:
            self.reserved[sku] -= qty

    def restock(self, sku, qty):
        self.stock[sku] = self.stock.get(sku, 0) + qty
        return self.stock[sku]


class OrderRepo:
    def __init__(self):
        self.orders = {}
        self.lines = {}
        self.by_customer = Counter()
        self._seq = 0

    def add(self, order_in, q):
        self._seq += 1
        oid = f"ORD-{self._seq:04d}"
        rec = {"order_id": oid, "customer": order_in.customer, "total": q["total"], "status": "placed"}
        if order_in.coupon:
            rec["coupon"] = order_in.coupon
        self.orders[oid] = rec
        self.lines[oid] = [(i.sku, i.qty) for i in order_in.items]
        self.by_customer[order_in.customer] += 1
        return rec

    def get(self, oid):
        return self.orders.get(oid)

    def set_status(self, oid, status, **extra):
        rec = self.orders[oid]
        rec["status"] = status
        rec.update(extra)
        return rec

    def top_customers(self, n=1):
        return self.by_customer.most_common(n)
