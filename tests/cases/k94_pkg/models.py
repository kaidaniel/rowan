"""Entities used by store.py and service.py."""
from dataclasses import dataclass, field

KIND_KEY = "kind"


@dataclass
class Item:
    sku: str
    price: float
    tags: list = field(default_factory=list)

    def label(self):
        return {"sku": self.sku, "kind": "item"}

    def with_tag(self, tag):
        self.tags.append(tag)
        return self


class Bundle(Item):
    def __init__(self, sku, items):
        super().__init__(sku, sum(i.price for i in items))
        self.items = list(items)

    def label(self):
        d = super().label()
        d["kind"] = "bundle"
        d["count"] = len(self.items)
        return d
