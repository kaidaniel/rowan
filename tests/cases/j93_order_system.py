"""A small order-management system in one file: layered settings with dotted-path access,
dataclass entities, repositories keyed by functions, an event bus with closures / bound
methods, a rule-chain pricer, a guarded state machine driven by a nested table, a
middleware router over a registry of routes, reporting with grouping / ranking / plugins,
a recursive org tree with a visitor, and three scenario runs whose results are checked at
the end.  Depth of the call graph: scenario -> services -> machine -> handlers -> bus ->
observers -> settings -> ...

Expectations (the `#@row` lines at the end) were written from the program text before
running the engine.  Where a comparison is open (amounts, ordering) the affected keys are
marked as possible (`?`); everything else follows from literal data.

Twenty-one of the checks needed engine work after the file was written: summaries
specialised by the classes of their arguments (so `svc.pricer.quote` resolves on `Pricer`),
one allocation context per summary variant and per call chain of any depth (so the quotes,
events and responses of the three scenarios stay distinct objects), `bool()` deciding on
literal truthiness (the feature flags), and budgets that degrade by a time limit instead of
small per-site counts (the scenarios make more calls than the old caps allowed)."""
import functools
import json
import sys
from collections import defaultdict
from dataclasses import dataclass, field

# ---------------------------------------------------------------- settings

DEFAULT_SETTINGS = {
    "currency": "EUR",
    "tax": {"rate": 0.2, "included": False},
    "shipping": {"free_over": 100, "base": 5},
    "features": {"loyalty": True, "coupons": True, "audit": False},
    "limits": {"max_lines": 10},
}


def deep_merge(base, override):
    out = {}
    for k, v in base.items():
        out[k] = v
    for k, v in override.items():
        if isinstance(v, dict) and isinstance(out.get(k), dict):
            out[k] = deep_merge(out[k], v)
        else:
            out[k] = v
    return out


class Settings:
    def __init__(self, data):
        self.data = data
        self.reads = []

    def get(self, path, default=None):
        cur = self.data
        for part in path.split("."):
            if not isinstance(cur, dict) or part not in cur:
                self.reads.append({"path": path, "hit": False})
                return default
            cur = cur[part]
        self.reads.append({"path": path, "hit": True})
        return cur

    def feature(self, name):
        return bool(self.get("features." + name, False))

    def section(self, name):
        return dict(self.data.get(name, {}))


# ---------------------------------------------------------------- entities


@dataclass
class Address:
    city: str
    country: str
    zip: str = ""

    def to_dict(self):
        d = {"city": self.city, "country": self.country}
        if self.zip:
            d["zip"] = self.zip
        return d


@dataclass
class Customer:
    id: int
    name: str
    address: Address
    tier: str = "basic"
    tags: list = field(default_factory=list)

    def is_vip(self):
        return self.tier == "gold"

    def to_dict(self):
        return {
            "id": self.id,
            "name": self.name,
            "tier": self.tier,
            "address": self.address.to_dict(),
            "tags": list(self.tags),
        }


@dataclass
class Product:
    sku: str
    price: float
    category: str
    stock: int = 0


@dataclass
class Line:
    product: Product
    qty: int

    @property
    def total(self):
        return self.product.price * self.qty

    def to_dict(self):
        return {"sku": self.product.sku, "qty": self.qty, "total": self.total}


class Order:
    def __init__(self, oid, customer):
        self.id = oid
        self.customer = customer
        self.lines = []
        self.state = "draft"
        self.history = []
        self.meta = {}

    def add_line(self, product, qty):
        line = Line(product, qty)
        self.lines.append(line)
        return line

    def subtotal(self):
        total = 0
        for ln in self.lines:
            total += ln.total
        return total

    def to_dict(self):
        d = {
            "id": self.id,
            "customer": self.customer.to_dict(),
            "lines": [ln.to_dict() for ln in self.lines],
            "state": self.state,
        }
        if self.meta:
            d["meta"] = dict(self.meta)
        return d


# ---------------------------------------------------------------- repositories


class Repo:
    def __init__(self, name, key_fn, index_fn=None):
        self.name = name
        self.key_fn = key_fn
        self.index_fn = index_fn
        self.items = {}
        self.index = defaultdict(list)
        self.events = []

    def add(self, obj):
        k = self.key_fn(obj)
        self.items[k] = obj
        if self.index_fn is not None:
            self.index[self.index_fn(obj)].append(obj)
        self.events.append({"op": "add", "key": k})
        return obj

    def get(self, k):
        return self.items[k]

    def where(self, pred):
        return [o for o in self.items.values() if pred(o)]

    def first(self, pred, default=None):
        for o in self.items.values():
            if pred(o):
                return o
        return default

    def count(self):
        return len(self.items)


def seed_customers(repo):
    repo.add(Customer(1, "ann", Address("Amsterdam", "NL", "1011"), tier="gold", tags=["early"]))
    repo.add(Customer(2, "bob", Address("Berlin", "DE")))
    repo.add(Customer(3, "cid", Address("Paris", "FR", "75001"), tier="silver"))
    return repo


def seed_products(repo):
    repo.add(Product("a", 10.0, "tools", stock=5))
    repo.add(Product("b", 25.0, "tools"))
    repo.add(Product("c", 4.0, "food", stock=100))
    return repo


# ---------------------------------------------------------------- event bus


class Bus:
    def __init__(self):
        self.handlers = defaultdict(list)
        self.log = []

    def on(self, topic, fn):
        self.handlers[topic].append(fn)
        return fn

    def emit(self, topic, payload):
        event = {"topic": topic, "payload": payload, "handled": 0}
        for fn in self.handlers.get(topic, []):
            fn(event)
            event["handled"] += 1
        self.log.append(event)
        return event


class Stats:
    def __init__(self):
        self.counts = {}
        self.totals = defaultdict(float)
        self.last = None

    def on_order(self, event):
        o = event["payload"]["order"]
        self.counts[o.state] = self.counts.get(o.state, 0) + 1
        self.totals[o.customer.tier] += o.subtotal()
        self.last = event
        event["stats"] = True


def make_auditor(settings, sink):
    def audit(event):
        if settings.feature("audit"):
            sink.append({"topic": event["topic"], "keys": sorted(event["payload"])})
            event["audited"] = True

    return audit


def make_notifier(prefix):
    def notify(event):
        event["note"] = prefix + event["topic"]

    return notify


# ---------------------------------------------------------------- pricing rules


class Rule:
    def applies(self, ctx):
        return True

    def apply(self, ctx, quote):
        return quote


class BaseRule(Rule):
    def apply(self, ctx, quote):
        quote["subtotal"] = ctx["order"].subtotal()
        quote["currency"] = ctx["settings"].get("currency")
        return quote


class TaxRule(Rule):
    def apply(self, ctx, quote):
        rate = ctx["settings"].get("tax.rate", 0)
        quote["tax"] = quote["subtotal"] * rate
        if ctx["settings"].get("tax.included"):
            quote["tax_included"] = True
        return quote


class ShippingRule(Rule):
    def applies(self, ctx):
        return not ctx["order"].meta.get("pickup")

    def apply(self, ctx, quote):
        free_over = ctx["settings"].get("shipping.free_over")
        if quote["subtotal"] >= free_over:
            quote["shipping"] = 0
            quote["free_shipping"] = True
        else:
            quote["shipping"] = ctx["settings"].get("shipping.base")
        return quote


class LoyaltyRule(Rule):
    def applies(self, ctx):
        return ctx["settings"].feature("loyalty") and ctx["order"].customer.is_vip()

    def apply(self, ctx, quote):
        quote["discount"] = quote["subtotal"] * 0.1
        quote["loyalty"] = {"tier": ctx["order"].customer.tier, "pct": 10}
        return quote


class CouponRule(Rule):
    def __init__(self, codes):
        self.codes = codes

    def applies(self, ctx):
        return ctx["settings"].feature("coupons") and "coupon" in ctx["order"].meta

    def apply(self, ctx, quote):
        code = ctx["order"].meta["coupon"]
        if code in self.codes:
            quote["coupon"] = {"code": code, "off": self.codes[code]}
            quote["discount"] = quote.get("discount", 0) + self.codes[code]
        else:
            quote["coupon_error"] = code
        return quote


class TotalRule(Rule):
    def apply(self, ctx, quote):
        quote["total"] = quote["subtotal"] + quote.get("tax", 0) + quote.get("shipping", 0) - quote.get("discount", 0)
        return quote


class Pricer:
    def __init__(self, rules):
        self.rules = list(rules)
        self.trace = []

    def quote(self, order, settings):
        ctx = {"order": order, "settings": settings}
        q = {}
        for rule in self.rules:
            if rule.applies(ctx):
                q = rule.apply(ctx, q)
                self.trace.append({"rule": rule, "keys": sorted(q)})
        return q


def default_rules():
    return [BaseRule(), TaxRule(), ShippingRule(), LoyaltyRule(), CouponRule({"SAVE5": 5, "TEN": 10}), TotalRule()]


# ---------------------------------------------------------------- state machine


def guard_amount(order, data):
    return "amount" in data


def guard_address(order, data):
    return order.customer.address.zip != ""


def on_submit(order, data):
    order.meta["submitted"] = True


def on_cancel(order, data):
    order.meta["reason"] = data.get("reason", "none")


def on_pay(order, data):
    order.meta["paid_at"] = data["at"]
    order.payment = {"amount": data["amount"], "method": data.get("method", "card")}


def on_ship(order, data):
    order.shipment = {"carrier": data["carrier"], "to": order.customer.address.to_dict()}


GUARDS = {"guard_amount": guard_amount, "guard_address": guard_address}
HANDLERS = {"on_submit": on_submit, "on_cancel": on_cancel, "on_pay": on_pay, "on_ship": on_ship}

TRANSITIONS = {
    "draft": {"submit": ("pending", None, "on_submit"), "cancel": ("cancelled", None, "on_cancel")},
    "pending": {"pay": ("paid", "guard_amount", "on_pay"), "cancel": ("cancelled", None, "on_cancel")},
    "paid": {"ship": ("shipped", "guard_address", "on_ship")},
}


class Machine:
    def __init__(self, bus):
        self.bus = bus
        self.fired = []

    def fire(self, order, event, data=None):
        data = data or {}
        row = TRANSITIONS.get(order.state, {})
        if event not in row:
            order.meta["rejected"] = event
            return False
        nxt, guard, action = row[event]
        if guard is not None and not GUARDS[guard](order, data):
            order.meta["blocked"] = event
            return False
        HANDLERS[action](order, data)
        order.history.append({"from": order.state, "to": nxt, "event": event})
        order.state = nxt
        self.fired.append({"order": order.id, "event": event})
        self.bus.emit("order." + event, {"order": order, "data": data})
        return True


# ---------------------------------------------------------------- services


class Services:
    def __init__(self, settings, customers, products, bus, pricer, machine):
        self.settings = settings
        self.customers = customers
        self.products = products
        self.bus = bus
        self.pricer = pricer
        self.machine = machine
        self.orders = {}

    def create_order(self, oid, customer_id, items):
        c = self.customers.get(customer_id)
        o = Order(oid, c)
        for sku, qty in items:
            o.add_line(self.products.get(sku), qty)
        self.orders[oid] = o
        return o

    def quote(self, oid):
        return self.pricer.quote(self.orders[oid], self.settings)

    def advance(self, oid, event, data=None):
        return self.machine.fire(self.orders[oid], event, data)

    def status(self, oid):
        o = self.orders[oid]
        return {"state": o.state, "steps": len(o.history), "meta": dict(o.meta)}


# ---------------------------------------------------------------- router / middleware


ROUTES = {}


def route(name):
    def deco(fn):
        ROUTES[name] = fn
        return fn

    return deco


@route("quote")
def r_quote(req, svc):
    return {"status": 200, "data": svc.quote(req["order"])}


@route("status")
def r_status(req, svc):
    return {"status": 200, "data": svc.status(req["order"])}


@route("advance")
def r_advance(req, svc):
    ok = svc.advance(req["order"], req["event"], req.get("data"))
    resp = {"status": 200, "ok": ok}
    if not ok:
        resp["status"] = 409
    return resp


def with_auth(users):
    def mw(nxt):
        def handler(req):
            token = req.get("token")
            if token not in users:
                return {"status": 401, "error": "auth"}
            req["user"] = users[token]
            return nxt(req)

        return handler

    return mw


def with_timing(nxt):
    def handler(req):
        resp = nxt(req)
        resp["timed"] = True
        return resp

    return handler


def with_json(nxt):
    def handler(req):
        resp = nxt(req)
        if "data" in resp:
            resp["body"] = json.dumps(resp.pop("data"))
        return resp

    return handler


def dispatch(svc, chain):
    def app(req):
        fn = ROUTES.get(req["action"])
        if fn is None:
            return {"status": 404, "error": "route"}
        return fn(req, svc)

    h = app
    for mw in reversed(chain):
        h = mw(h)
    return h


# ---------------------------------------------------------------- reporting


def sales_report(orders, settings):
    by_tier = defaultdict(list)
    by_state = {}
    for o in orders.values():
        by_tier[o.customer.tier].append(o)
        by_state[o.state] = by_state.get(o.state, 0) + 1
    report = {"n": len(orders), "by_state": by_state, "tiers": {}}
    for tier, group in by_tier.items():
        report["tiers"][tier] = {"count": len(group), "revenue": sum(o.subtotal() for o in group)}
    ranked = sorted(orders.values(), key=lambda o: o.subtotal(), reverse=True)
    report["top"] = [{"rank": i + 1, "id": o.id} for i, o in enumerate(ranked[:2])]
    if settings.feature("loyalty"):
        report["loyalty"] = {t: len(v) for t, v in by_tier.items() if t == "gold"}
    return report


PLUGINS = {}


def plugin(name, requires=()):
    def deco(cls):
        PLUGINS[name] = (cls, tuple(requires))
        return cls

    return deco


@plugin("summary")
class SummaryPlugin:
    def run(self, report, ctx):
        report["summary"] = {"orders": report["n"], "states": sorted(report["by_state"])}


@plugin("vip", requires=("summary",))
class VipPlugin:
    def run(self, report, ctx):
        if "loyalty" in report:
            report["summary"]["vip"] = report["loyalty"].get("gold", 0)
        else:
            report["summary"]["vip"] = 0
        report["summary"]["currency"] = ctx["settings"].get("currency")


@plugin("audit", requires=("summary",))
class AuditPlugin:
    def run(self, report, ctx):
        if ctx["settings"].feature("audit"):
            report["audit"] = {"entries": len(ctx["audit"])}


def run_plugins(report, ctx, names):
    done = []
    for name in names:
        cls, requires = PLUGINS[name]
        for r in requires:
            if r not in done:
                raise RuntimeError("plugin " + name + " needs " + r)
        cls().run(report, ctx)
        done.append(name)
    return done


# ---------------------------------------------------------------- org tree


@dataclass
class OrgNode:
    name: str
    children: list = field(default_factory=list)
    attrs: dict = field(default_factory=dict)


ORG = {
    "name": "root",
    "attrs": {"region": "eu"},
    "children": [
        {"name": "sales", "children": [{"name": "nl", "attrs": {"lead": "ann"}}]},
        {"name": "ops"},
    ],
}


def build_org(spec):
    node = OrgNode(spec["name"], attrs=dict(spec.get("attrs", {})))
    for child in spec.get("children", []):
        node.children.append(build_org(child))
    return node


class Visitor:
    def __init__(self):
        self.seen = {}
        self.leaves = []

    def visit(self, node, depth=0):
        self.seen[node.name] = {"depth": depth, "attrs": dict(node.attrs)}
        if not node.children:
            self.leaves.append(node.name)
        for ch in node.children:
            self.visit(ch, depth + 1)
        return self.seen


def org_paths(node, prefix=""):
    here = prefix + node.name
    out = {here: node}
    for ch in node.children:
        out.update(org_paths(ch, here + "/"))
    return out


# ---------------------------------------------------------------- scenarios


def wire(overrides):
    settings = Settings(deep_merge(DEFAULT_SETTINGS, overrides))
    bus = Bus()
    stats = Stats()
    audit_sink = []
    bus.on("order.submit", stats.on_order)
    bus.on("order.pay", stats.on_order)
    bus.on("order.pay", make_auditor(settings, audit_sink))
    bus.on("order.ship", make_notifier("sent:"))
    customers = seed_customers(Repo("customers", lambda c: c.id, lambda c: c.tier))
    products = seed_products(Repo("products", lambda p: p.sku, lambda p: p.category))
    pricer = Pricer(default_rules())
    machine = Machine(bus)
    svc = Services(settings, customers, products, bus, pricer, machine)
    return {"settings": settings, "bus": bus, "stats": stats, "audit": audit_sink, "svc": svc, "pricer": pricer, "machine": machine}


def run_scenario(overrides, coupon=None, pickup=False):
    w = wire(overrides)
    svc = w["svc"]
    settings = w["settings"]
    o1 = svc.create_order(1, 1, [("a", 2), ("b", 1)])
    if coupon:
        o1.meta["coupon"] = coupon
    if pickup:
        o1.meta["pickup"] = True
    q1 = svc.quote(1)
    svc.advance(1, "submit")
    svc.advance(1, "pay", {"amount": 10, "at": 1})
    svc.advance(1, "ship", {"carrier": "dhl"})
    o2 = svc.create_order(2, 2, [("c", 3)])
    q2 = svc.quote(2)
    svc.advance(2, "submit")
    svc.advance(2, "cancel", {"reason": "x"})
    svc.advance(2, "pay", {"amount": 1, "at": 2})
    o3 = svc.create_order(3, 3, [("a", 1)])
    svc.advance(3, "submit")
    svc.advance(3, "pay", {"at": 3})
    users = {"t1": {"name": "ann", "role": "admin"}}
    handler = dispatch(svc, [with_timing, with_auth(users), with_json])
    resp = handler({"action": "quote", "token": "t1", "order": 1})
    denied = handler({"action": "quote", "token": "zz"})
    unknown = handler({"action": "nope", "token": "t1"})
    adv = handler({"action": "advance", "token": "t1", "order": 3, "event": "cancel"})
    status = handler({"action": "status", "token": "t1", "order": 2})
    report = sales_report(svc.orders, settings)
    run_plugins(report, {"settings": settings, "audit": w["audit"]}, ["summary", "vip", "audit"])
    org = build_org(ORG)
    seen = Visitor().visit(org)
    paths = org_paths(org)
    return {
        "w": w,
        "settings": settings,
        "o1": o1,
        "o2": o2,
        "o3": o3,
        "q1": q1,
        "q2": q2,
        "resp": resp,
        "denied": denied,
        "unknown": unknown,
        "adv": adv,
        "status": status,
        "report": report,
        "org": org,
        "seen": seen,
        "paths": paths,
    }


S1 = run_scenario({"tax": {"rate": 0.25}, "features": {"audit": True}, "extra": 1}, coupon="SAVE5")
S2 = run_scenario({}, pickup=True)
S3 = run_scenario({"features": {"loyalty": False}}, coupon="NOPE")

# ---------------------------------------------------------------- checks: settings

s1_data = S1["settings"].data                #@row s1_data: present={currency,tax,shipping,features,limits,extra} definite={currency,tax,shipping,features,limits,extra} closed
s1_tax = s1_data["tax"]                      #@row s1_tax: present={rate,included} definite={rate,included} closed
s1_features = s1_data["features"]            #@row s1_features: present={loyalty,coupons,audit} definite={loyalty,coupons,audit} closed
s2_data = S2["settings"].data                #@row s2_data: present={currency,tax,shipping,features,limits} definite={currency,tax,shipping,features,limits} closed
s1_settings = S1["settings"]                 #@row s1_settings: present={data,reads} definite={data,reads} closed
read0 = s1_settings.reads[0]                 #@row read0: present={path,hit} definite={path,hit} closed
shipping_section = s1_settings.section("shipping")  #@row shipping_section: present={free_over,base} definite={free_over,base} closed
missing_section = s1_settings.section("nope")       #@row missing_section: present={} definite={} closed

# ---------------------------------------------------------------- checks: entities and repos

o1 = S1["o1"]                                #@row o1: present={id,customer,lines,state,history,meta,payment,shipment} definite={id,customer,lines,state,history,meta,payment,shipment} closed
o1_meta = o1.meta                            #@row o1_meta: present={coupon,submitted,paid_at} definite={coupon,submitted,paid_at} closed
o1_payment = o1.payment                      #@row o1_payment: present={amount,method} definite={amount,method} closed
o1_shipment = o1.shipment                    #@row o1_shipment: present={carrier,to} definite={carrier,to} closed
o1_to = o1_shipment["to"]                    #@row o1_to: present={city,country,zip} definite={city,country,zip} closed
o1_hist = o1.history[2]                      #@row o1_hist: present={from,to,event} definite={from,to,event} closed
o1_line = o1.lines[1]                        #@row o1_line: present={product,qty} definite={product,qty} closed
o1_product = o1_line.product                 #@row o1_product: present={sku,price,category,stock} definite={sku,price,category,stock} closed
o1_customer = o1.customer                    #@row o1_customer: present={id,name,address,tier,tags} definite={id,name,address,tier,tags} closed
o1_dict = o1.to_dict()                       #@row o1_dict: present={id,customer,lines,state,meta} definite={id,customer,lines,state,meta} closed
o1_dict_customer = o1_dict["customer"]       #@row o1_dict_customer: present={id,name,tier,address,tags} definite={id,name,tier,address,tags} closed
o1_dict_addr = o1_dict_customer["address"]   #@row o1_dict_addr: present={city,country,zip} definite={city,country,zip} closed
o1_dict_line = o1_dict["lines"][0]           #@row o1_dict_line: present={sku,qty,total} definite={sku,qty,total} closed
# Order 2: submitted, cancelled, then a rejected "pay" (no transitions from "cancelled").
o2 = S1["o2"]                                #@row o2: present={id,customer,lines,state,history,meta} definite={id,customer,lines,state,history,meta} closed
o2_meta = o2.meta                            #@row o2_meta: present={submitted,reason,rejected} definite={submitted,reason,rejected} closed
o2_addr = o2.customer.address.to_dict()      #@row o2_addr: present={city,country} definite={city,country} closed
# Order 3: "pay" without an amount is blocked by the guard.
o3 = S1["o3"]                                #@row o3: present={id,customer,lines,state,history,meta} definite={id,customer,lines,state,history,meta} closed
o3_meta = o3.meta                            #@row o3_meta: present={submitted,blocked,reason} definite={submitted,blocked,reason} closed
customers = S1["w"]["svc"].customers.items   #@row customers: present={1,2,3} definite={1,2,3} closed
cust_index = S1["w"]["svc"].customers.index  #@row cust_index: present={gold,basic,silver} definite={gold,basic,silver} closed
prod_index = S1["w"]["svc"].products.index   #@row prod_index: present={tools,food} definite={tools,food} closed
repo_event = S1["w"]["svc"].products.events[2]  #@row repo_event: present={op,key} definite={op,key} closed
orders = S1["w"]["svc"].orders               #@row orders: present={1,2,3} definite={1,2,3} closed

# ---------------------------------------------------------------- checks: quotes

# Scenario 1: gold customer, valid coupon, shipping applies (the amount comparison is open).
q1 = S1["q1"]                                #@row q1: present={subtotal,currency,tax,shipping,free_shipping,discount,loyalty,coupon,total} definite={subtotal,currency,tax,shipping,discount,loyalty,coupon,total} closed
q1_coupon = q1["coupon"]                     #@row q1_coupon: present={code,off} definite={code,off} closed
q1_loyalty = q1["loyalty"]                   #@row q1_loyalty: present={tier,pct} definite={tier,pct} closed
# Scenario 1, order 2: basic customer, no coupon.
q2 = S1["q2"]                                #@row q2: present={subtotal,currency,tax,shipping,free_shipping,total} definite={subtotal,currency,tax,shipping,total} closed
# Scenario 2: pickup, so no shipping rule; loyalty still applies; no coupon.
q1_s2 = S2["q1"]                             #@row q1_s2: present={subtotal,currency,tax,discount,loyalty,total} definite={subtotal,currency,tax,discount,loyalty,total} closed
# Scenario 3: loyalty off, unknown coupon code.
q1_s3 = S3["q1"]                             #@row q1_s3: present={subtotal,currency,tax,shipping,free_shipping,coupon_error,total} definite={subtotal,currency,tax,shipping,coupon_error,total} closed
q1["tax_included"]                           #@ missing tax_included
q1_s3["discount"]                            #@ missing discount
q1["free_shipping"]                          #@ possibly free_shipping
trace0 = S1["w"]["pricer"].trace[0]          #@row trace0: present={rule,keys} definite={rule,keys} closed

# ---------------------------------------------------------------- checks: bus and stats

stats = S1["w"]["stats"]                     #@row stats: present={counts,totals,last} definite={counts,totals,last} closed
counts = stats.counts                        #@row counts: present={pending,paid} definite={pending,paid} closed
totals = stats.totals                        #@row totals: present={gold,basic,silver} definite={gold,basic,silver} closed
last = stats.last                            #@row last: present={topic,payload,handled,stats} definite={topic,payload,handled,stats} closed
pay_event = S1["w"]["bus"].log[1]            #@row pay_event: present={topic,payload,handled,stats,audited} definite={topic,payload,handled,stats,audited} closed
ship_event = S1["w"]["bus"].log[2]           #@row ship_event: present={topic,payload,handled,note} definite={topic,payload,handled,note} closed
cancel_event = S1["w"]["bus"].log[4]         #@row cancel_event: present={topic,payload,handled} definite={topic,payload,handled} closed
audit0 = S1["w"]["audit"][0]                 #@row audit0: present={topic,keys} definite={topic,keys} closed
pay_event_s2 = S2["w"]["bus"].log[1]         #@row pay_event_s2: present={topic,payload,handled,stats} definite={topic,payload,handled,stats} closed
handlers = S1["w"]["bus"].handlers           #@row handlers: present={order.submit,order.pay,order.ship} definite={order.submit,order.pay,order.ship} closed
fired = S1["w"]["machine"].fired[0]          #@row fired: present={order,event} definite={order,event} closed

# ---------------------------------------------------------------- checks: router

resp = S1["resp"]                            #@row resp: present={status,body,timed} definite={status,body,timed} closed
denied = S1["denied"]                        #@row denied: present={status,error,timed} definite={status,error,timed} closed
unknown = S1["unknown"]                      #@row unknown: present={status,error,timed} definite={status,error,timed} closed
# Order 3 is pending: "cancel" is a valid transition, so ok is True and the status stays 200.
adv = S1["adv"]                              #@row adv: present={status,ok,timed} definite={status,ok,timed} closed
status = S1["status"]                        #@row status: present={status,body,timed} definite={status,body,timed} closed
resp["data"]                                 #@ missing data
denied["body"]                               #@ missing body

# ---------------------------------------------------------------- checks: report and plugins

report = S1["report"]                        #@row report: present={n,by_state,tiers,top,loyalty,summary,audit} definite={n,by_state,tiers,top,loyalty,summary,audit} closed
by_state = report["by_state"]                #@row by_state: present={shipped,cancelled} definite={shipped,cancelled} closed
tiers = report["tiers"]                      #@row tiers: present={gold,basic,silver} definite={gold,basic,silver} closed
tier_gold = tiers["gold"]                    #@row tier_gold: present={count,revenue} definite={count,revenue} closed
top0 = report["top"][0]                      #@row top0: present={rank,id} definite={rank,id} closed
loyalty = report["loyalty"]                  #@row loyalty: present={gold} definite={gold} closed
summary = report["summary"]                  #@row summary: present={orders,states,vip,currency} definite={orders,states,vip,currency} closed
audit_section = report["audit"]              #@row audit_section: present={entries} definite={entries} closed
report_s3 = S3["report"]                     #@row report_s3: present={n,by_state,tiers,top,summary} definite={n,by_state,tiers,top,summary} closed
report_s3["loyalty"]                         #@ missing loyalty
report_s3["audit"]                           #@ missing audit

# ---------------------------------------------------------------- checks: org tree

org = S1["org"]                              #@row org: present={name,children,attrs} definite={name,children,attrs} closed
org_attrs = org.attrs                        #@row org_attrs: present={region} definite={region} closed
sales = org.children[0]                      #@row sales: present={name,children,attrs} definite={name,children,attrs} closed
sales_attrs = sales.attrs                    #@row sales_attrs: present={} definite={} closed
nl = sales.children[0]                       #@row nl: present={name,children,attrs} definite={name,children,attrs} closed
nl_attrs = nl.attrs                          #@row nl_attrs: present={lead} definite={lead} closed
seen = S1["seen"]                            #@row seen: present={root,sales,nl,ops} definite={root,sales,nl,ops} closed
seen_nl = seen["nl"]                         #@row seen_nl: present={depth,attrs} definite={depth,attrs} closed
paths = S1["paths"]                          #@row paths: present={root,root/sales,root/sales/nl,root/ops} definite={root,root/sales,root/sales/nl,root/ops} closed
ops = paths["root/ops"]                      #@row ops: present={name,children,attrs} definite={name,children,attrs} closed

# ---------------------------------------------------------------- checks: wire format

payload = json.loads(json.dumps(o1.to_dict()))  #@row payload: present={} definite={} open
payload["id"]                                #@ input id
payload["customer"]["tier"]                  #@ input tier
#@schema json.loads@L827: always={id,customer}
#@schema json.loads@L827['customer']: always={tier}
