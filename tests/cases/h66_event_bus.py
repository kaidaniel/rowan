"""Event bus: handlers (a closure, a bound method, a partial) registered per topic receive the
topic and the event; they mutate the event, a captured context and their own objects.

Expectations were written from the program text before running the engine."""
import functools


class Bus:
    def __init__(self):
        self.handlers = {}
        self.log = []

    def on(self, topic, fn):
        self.handlers.setdefault(topic, []).append(fn)
        return fn

    def emit(self, topic, event):
        event["topic"] = topic
        for fn in self.handlers.get(topic, []):
            fn(topic, event)
        self.log.append(event)
        return event


class Metrics:
    def __init__(self):
        self.counts = {}
        self.last = None

    def observe(self, topic, event):
        self.counts[topic] = self.counts.get(topic, 0) + 1
        self.last = event
        event["observed"] = True


def make_enricher(ctx):
    def enrich(topic, event):
        event["tenant"] = ctx["tenant"]
        ctx["enriched"] = ctx.get("enriched", 0) + 1
    return enrich


def audit(sink, topic, event):
    sink["entries"].append({"topic": topic, "id": event["id"]})
    event["audited"] = True


bus = Bus()
metrics = Metrics()
ctx = {"tenant": "acme"}
sink = {"entries": []}
bus.on("order.created", make_enricher(ctx))
bus.on("order.created", metrics.observe)
bus.on("order.paid", metrics.observe)
bus.on("order.paid", functools.partial(audit, sink))
handlers = bus.handlers                   #@row handlers: present={order.created,order.paid} definite={order.created,order.paid} closed

# "order.created": enrich then observe.
created = bus.emit("order.created", {"id": 1, "total": 10})  #@row created: present={id,total,topic,tenant,observed} definite={id,total,topic,tenant,observed} closed
_ = 0                                     #@row ctx: present={tenant,enriched} definite={tenant,enriched} closed

# "order.paid": observe then audit.
paid = bus.emit("order.paid", {"id": 1})  #@row paid: present={id,topic,observed,audited} definite={id,topic,observed,audited} closed
entry = sink["entries"][0]                #@row entry: present={topic,id} definite={topic,id} closed

# No handler registered: only the topic is added.
shipped = bus.emit("order.shipped", {"id": 1, "carrier": "ups"})  #@row shipped: present={id,carrier,topic} definite={id,carrier,topic} closed

_ = 0                                     #@row metrics: present={counts,last} definite={counts,last} closed
counts = metrics.counts                   #@row counts: present={order.created,order.paid} definite={order.created,order.paid} closed
last = metrics.last                       #@row last: present={id,topic,observed,audited} definite={id,topic,observed,audited} closed
logged = bus.log[0]                       #@row logged: present={id,total,topic,tenant,observed} definite={id,total,topic,tenant,observed} closed
last["audited"]                           #@ ok audited
created["audited"]                        #@ missing audited
shipped["observed"]                       #@ missing observed
