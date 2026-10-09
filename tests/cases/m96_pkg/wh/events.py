"""An event bus with per-topic handler lists, a wildcard list, an audit sink, and handlers
of three kinds: closures, bound methods and partial-like wrappers."""
from collections import defaultdict


class Bus:
    def __init__(self):
        self.handlers = defaultdict(list)
        self.wildcard = []
        self.log = []
        self.counts = {}

    def on(self, topic, fn):
        if topic == "*":
            self.wildcard.append(fn)
        else:
            self.handlers[topic].append(fn)
        return fn

    def emit(self, topic, payload):
        event = {"topic": topic, "payload": payload, "handled": 0}
        for fn in self.handlers.get(topic, []):
            fn(event)
            event["handled"] += 1
        for fn in self.wildcard:
            fn(event)
        self.counts[topic] = self.counts.get(topic, 0) + 1
        self.log.append(event)
        return event

    def last(self, topic):
        for e in reversed(self.log):
            if e["topic"] == topic:
                return e
        return None


class Tally:
    def __init__(self):
        self.by_topic = defaultdict(int)
        self.last = None

    def on_any(self, event):
        self.by_topic[event["topic"]] += 1
        self.last = event
        event["tallied"] = True


def make_auditor(cfg, sink):
    def audit(event):
        if cfg.feature("audit"):
            sink.append({"topic": event["topic"], "keys": sorted(event["payload"])})
            event["audited"] = True

    return audit


def make_tagger(tag):
    def tagger(event):
        event["tag"] = tag
        event["payload"]["tagged_by"] = tag

    return tagger


def wire_bus(cfg, sink):
    bus = Bus()
    tally = Tally()
    bus.on("*", tally.on_any)
    bus.on("lot.received", make_auditor(cfg, sink))
    bus.on("shipment.shipped", make_auditor(cfg, sink))
    bus.on("shipment.shipped", make_tagger("ship"))
    return {"bus": bus, "tally": tally}
