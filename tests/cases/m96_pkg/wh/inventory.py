"""Stock on hand: lots per product and per location, FIFO consumption that splits lots,
reservations, a move log, an index helper keyed by functions, and counts by zone."""
from collections import defaultdict

from .models import Lot, Perishable
from .util.text import key_for


class Index:
    def __init__(self, name, key_fn):
        self.name = name
        self.key_fn = key_fn
        self.groups = defaultdict(list)
        self.n = 0

    def add(self, obj):
        self.groups[self.key_fn(obj)].append(obj)
        self.n += 1
        return obj

    def remove(self, obj):
        k = self.key_fn(obj)
        if obj in self.groups[k]:
            self.groups[k].remove(obj)
            self.n -= 1
        return obj

    def keys(self):
        return sorted(self.groups)

    def summary(self):
        return {"name": self.name, "groups": len(self.groups), "n": self.n}


class Inventory:
    def __init__(self, cfg, bus):
        self.cfg = cfg
        self.bus = bus
        self.products = {}
        self.locations = {}
        self.lots = {}
        self.by_product = Index("by_product", lambda lot: lot.product.sku)
        self.by_location = Index("by_location", lambda lot: lot.location.code)
        self.reservations = {}
        self.moves = []
        self.seq = 0

    def add_product(self, p):
        self.products[p.sku] = p
        return p

    def add_location(self, loc):
        self.locations[loc.code] = loc
        return loc

    def next_lot_id(self):
        self.seq += 1
        return "L%03d" % self.seq

    def receive(self, sku, qty, loc_code, expires=0, meta=None):
        product = self.products[sku]
        location = self.locations[loc_code]
        lot = Lot(self.next_lot_id(), product, qty, location, expires, dict(meta or {}))
        self.lots[lot.lot_id] = lot
        self.by_product.add(lot)
        self.by_location.add(lot)
        self.moves.append({"op": "in", "lot": lot.lot_id, "qty": qty, "to": loc_code})
        self.bus.emit("lot.received", {"lot": lot.to_dict(), "sku": sku})
        return lot

    def on_hand(self, sku):
        return sum(lot.qty for lot in self.by_product.groups.get(sku, []))

    def available(self, sku):
        return self.on_hand(sku) - self.reservations.get(sku, 0)

    def reserve(self, sku, qty):
        self.reservations[sku] = self.reservations.get(sku, 0) + qty
        return {"sku": sku, "reserved": self.reservations[sku]}

    def release(self, sku, qty):
        cur = self.reservations.get(sku, 0)
        if cur <= qty:
            self.reservations.pop(sku, None)
        else:
            self.reservations[sku] = cur - qty
        return self.reservations.get(sku, 0)

    def lots_fifo(self, sku):
        return sorted(self.by_product.groups.get(sku, []), key=lambda lot: lot.lot_id)

    def consume(self, sku, qty, reason):
        """Take `qty` of `sku` out of the oldest lots; returns the picks made."""
        picks = []
        remaining = qty
        for lot in self.lots_fifo(sku):
            if remaining <= 0:
                break
            take = lot.qty if lot.qty <= remaining else remaining
            lot.qty -= take
            remaining -= take
            picks.append({"lot": lot.lot_id, "qty": take, "loc": lot.location.code})
            self.moves.append({"op": "out", "lot": lot.lot_id, "qty": take, "why": reason})
            if lot.qty == 0:
                self.by_product.remove(lot)
                self.by_location.remove(lot)
                del self.lots[lot.lot_id]
        result = {"sku": sku, "requested": qty, "picks": picks, "short": remaining}
        if remaining > 0:
            result["partial"] = True
        return result

    def move(self, lot_id, to_code):
        lot = self.lots[lot_id]
        src = lot.location.code
        self.by_location.remove(lot)
        lot.location = self.locations[to_code]
        self.by_location.add(lot)
        rec = {"op": "move", "lot": lot_id, "from": src, "to": to_code}
        self.moves.append(rec)
        return rec

    def count_by_zone(self):
        out = defaultdict(int)
        for lot in self.lots.values():
            out[lot.location.zone] += lot.qty
        return dict(out)

    def perishables(self):
        return [lot for lot in self.lots.values() if lot.is_perishable()]

    def snapshot(self):
        snap = {"products": sorted(self.products), "lots": {}, "zones": self.count_by_zone(), "moves": len(self.moves)}
        for lot_id, lot in self.lots.items():
            snap["lots"][lot_id] = lot.to_dict()
        if self.reservations:
            snap["reserved"] = dict(self.reservations)
        return snap

    def locate(self, sku):
        found = {}
        for lot in self.by_product.groups.get(sku, []):
            found[lot.location.code] = found.get(lot.location.code, 0) + lot.qty
        return found

    def product_keys(self):
        return [key_for("product", sku) for sku in sorted(self.products)]
