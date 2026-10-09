"""Shipping: a carrier registry, rate rules, labels, a tracking state machine driven by a
transition table with guards, and hooks run around state changes."""
from .models import Carrier, Shipment
from .util.text import money

CARRIERS = {}


def register_carrier(code, name, zones):
    c = Carrier(code, name, zones)
    CARRIERS[code] = c
    return c


def pick_carrier(cfg, zone):
    code = cfg.get("shipping.default_carrier", "dhl")
    default = CARRIERS.get(code)
    if default is not None and default.serves(zone):
        return default
    for c in CARRIERS.values():
        if c.serves(zone):
            return c
    return default


class RateRule:
    def apply(self, ctx, rate):
        return rate


class BaseRate(RateRule):
    def apply(self, ctx, rate):
        rate["base_cents"] = 500 + ctx["weight_g"] // 1000 * 100
        rate["currency"] = "EUR"
        return rate


class FreeShipping(RateRule):
    def apply(self, ctx, rate):
        if ctx["weight_g"] < ctx["cfg"].get("shipping.free_over_g", 20000) and ctx["order"].get("rush"):
            rate["discount_cents"] = rate["base_cents"]
            rate["free"] = True
        return rate


class Insurance(RateRule):
    def apply(self, ctx, rate):
        if ctx["value_cents"] >= ctx["cfg"].get("shipping.insure_over", 50000):
            rate["insurance_cents"] = ctx["value_cents"] // 100
        return rate


class Total(RateRule):
    def apply(self, ctx, rate):
        rate["total_cents"] = rate["base_cents"] - rate.get("discount_cents", 0) + rate.get("insurance_cents", 0)
        rate["total"] = money(rate["total_cents"])
        return rate


RATE_RULES = [BaseRate(), FreeShipping(), Insurance(), Total()]


def rate_for(cfg, shipment, value_cents):
    ctx = {"cfg": cfg, "order": shipment.order, "weight_g": shipment.weight(), "value_cents": value_cents}
    rate = {}
    for rule in RATE_RULES:
        rate = rule.apply(ctx, rate)
    return rate


TRANSITIONS = {
    "new": {"pack": ("packed", None), "cancel": ("cancelled", None)},
    "packed": {"label": ("labelled", "has_carrier"), "cancel": ("cancelled", None)},
    "labelled": {"ship": ("shipped", "has_label")},
    "shipped": {"deliver": ("delivered", None)},
}


def guard_has_carrier(shipment):
    return shipment.carrier is not None


def guard_has_label(shipment):
    return shipment.label is not None


GUARDS = {"has_carrier": guard_has_carrier, "has_label": guard_has_label}


class Tracker:
    def __init__(self, bus):
        self.bus = bus
        self.hooks = {"before": [], "after": []}
        self.shipments = {}
        self.seq = 0

    def hook(self, when, fn):
        self.hooks[when].append(fn)
        return fn

    def create(self, order, carrier=None):
        self.seq += 1
        s = Shipment("S%02d" % self.seq, order, carrier)
        self.shipments[s.ship_id] = s
        return s

    def fire(self, shipment, event):
        row = TRANSITIONS.get(shipment.state, {})
        if event not in row:
            shipment.meta["rejected"] = event
            return {"ok": False, "why": "no transition", "state": shipment.state}
        nxt, guard = row[event]
        if guard is not None and not GUARDS[guard](shipment):
            shipment.meta["blocked"] = guard
            return {"ok": False, "why": "guard", "guard": guard}
        for fn in self.hooks["before"]:
            fn(shipment, event)
        shipment.history.append({"from": shipment.state, "to": nxt, "event": event})
        shipment.state = nxt
        for fn in self.hooks["after"]:
            fn(shipment, event)
        self.bus.emit("shipment." + event, {"shipment": shipment.to_dict(), "event": event})
        return {"ok": True, "state": nxt}

    def pack(self, shipment, boxes):
        shipment.boxes = list(boxes)
        return self.fire(shipment, "pack")

    def label(self, shipment, rate):
        if shipment.carrier is None:
            return self.fire(shipment, "label")
        shipment.label = {"carrier": shipment.carrier.code, "to": shipment.order["customer"]["address"], "cost": rate["total"], "boxes": len(shipment.boxes)}
        if rate.get("insurance_cents"):
            shipment.label["insured"] = True
        result = self.fire(shipment, "label")
        if result["ok"]:
            shipment.carrier.stats["shipments"] += 1
        return result

    def states(self):
        out = {}
        for s in self.shipments.values():
            out[s.state] = out.get(s.state, 0) + 1
        return out


def stamp_meta(shipment, event):
    shipment.meta["last_event"] = event


def count_events(counts):
    def hook(shipment, event):
        counts[event] = counts.get(event, 0) + 1

    return hook
