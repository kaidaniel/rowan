"""A store keyed by SKU, with a module-level registry of stores and an index by kind."""
from collections import defaultdict

import models
from models import KIND_KEY

STORES = {}


class Store:
    def __init__(self, name):
        self.name = name
        self.items = {}
        self.by_kind = defaultdict(list)
        self.log = []
        STORES[name] = self

    def add(self, item):
        self.items[item.sku] = item
        lab = item.label()
        self.by_kind[lab[KIND_KEY]].append(item)
        self.log.append({"op": "add", "sku": item.sku, KIND_KEY: lab[KIND_KEY]})
        return item

    def get(self, sku):
        return self.items[sku]

    def summary(self):
        return {"name": self.name, "n": len(self.items), "kinds": sorted(self.by_kind)}


def open_store(name):
    if name in STORES:
        return STORES[name]
    return Store(name)
