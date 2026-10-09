"""Entities; imports a helper two levels down through a relative import."""
from .util.money import cents


class Item:
    def __init__(self, sku, price):
        self.sku = sku
        self.price = cents(price)


class Cart:
    def __init__(self, owner):
        self.owner = owner
        self.lines = []

    def add(self, item, qty):
        self.lines.append({"item": item, "qty": qty})
        return self

    def total(self):
        return sum(line["item"].price * line["qty"] for line in self.lines)
