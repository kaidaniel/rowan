"""Entities of the warehouse: dataclasses with `to_dict` / `from_dict`, a dataclass
subclass, `__post_init__`, class-level constants and a few derived properties."""
from dataclasses import dataclass, field

from .util.text import key_for

UNITS = {"ea": 1, "box": 12, "pallet": 480}


@dataclass
class Product:
    sku: str
    name: str
    unit: str = "ea"
    weight_g: int = 0
    tags: list = field(default_factory=list)
    attrs: dict = field(default_factory=dict)

    def key(self):
        return key_for("product", self.sku)

    def per_unit(self):
        return UNITS[self.unit]

    def to_dict(self):
        d = {"sku": self.sku, "name": self.name, "unit": self.unit, "weight_g": self.weight_g}
        if self.tags:
            d["tags"] = list(self.tags)
        if self.attrs:
            d["attrs"] = dict(self.attrs)
        return d

    @classmethod
    def from_dict(cls, d):
        return cls(d["sku"], d["name"], d.get("unit", "ea"), d.get("weight_g", 0), list(d.get("tags", [])), dict(d.get("attrs", {})))


@dataclass
class Perishable(Product):
    shelf_days: int = 30

    def to_dict(self):
        d = super().to_dict()
        d["shelf_days"] = self.shelf_days
        return d


@dataclass
class Location:
    code: str
    zone: str
    capacity: int = 100
    attrs: dict = field(default_factory=dict)

    def __post_init__(self):
        self.aisle = self.code.split("-")[0]

    def key(self):
        return key_for("loc", self.code)

    def to_dict(self):
        return {"code": self.code, "zone": self.zone, "capacity": self.capacity, "aisle": self.aisle}


@dataclass
class Lot:
    lot_id: str
    product: Product
    qty: int
    location: Location
    expires: int = 0
    meta: dict = field(default_factory=dict)

    def is_perishable(self):
        return isinstance(self.product, Perishable)

    def to_dict(self):
        d = {"lot": self.lot_id, "sku": self.product.sku, "qty": self.qty, "loc": self.location.code}
        if self.expires:
            d["expires"] = self.expires
        return d


class Shipment:
    STATES = ("new", "packed", "labelled", "shipped", "delivered")

    def __init__(self, ship_id, order, carrier=None):
        self.ship_id = ship_id
        self.order = order
        self.carrier = carrier
        self.state = "new"
        self.boxes = []
        self.history = []
        self.label = None
        self.meta = {}

    def weight(self):
        return sum(b["weight_g"] for b in self.boxes)

    def to_dict(self):
        d = {"id": self.ship_id, "order": self.order["id"], "state": self.state, "boxes": len(self.boxes)}
        if self.carrier is not None:
            d["carrier"] = self.carrier.code
        if self.label is not None:
            d["label"] = dict(self.label)
        return d


class Carrier:
    def __init__(self, code, name, zones):
        self.code = code
        self.name = name
        self.zones = list(zones)
        self.stats = {"shipments": 0}

    def serves(self, zone):
        return zone in self.zones

    def describe(self):
        return {"code": self.code, "name": self.name, "zones": list(self.zones)}


def order_record(oid, customer, lines, priority="normal"):
    rec = {"id": oid, "customer": customer, "lines": list(lines), "priority": priority, "status": "open"}
    if priority == "rush":
        rec["rush"] = True
    return rec
