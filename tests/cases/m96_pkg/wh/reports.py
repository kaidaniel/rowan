"""Reporting over the inventory, the shipments and the bus log: grouping, ranking, a plugin
chain, and a JSON round trip of the result."""
import json
from collections import Counter, defaultdict

from .util.paths import pick

PLUGINS = {}


def plugin(name, after=()):
    def deco(fn):
        PLUGINS[name] = (fn, tuple(after))
        return fn

    return deco


def stock_report(inventory):
    by_zone = inventory.count_by_zone()
    by_sku = {}
    for lot in inventory.lots.values():
        by_sku[lot.product.sku] = by_sku.get(lot.product.sku, 0) + lot.qty
    ranked = sorted(by_sku.items(), key=lambda kv: kv[1], reverse=True)
    report = {"zones": by_zone, "skus": by_sku, "top": [{"sku": sku, "qty": qty} for sku, qty in ranked[:2]], "lots": len(inventory.lots)}
    if inventory.reservations:
        report["reserved"] = dict(inventory.reservations)
    perishable = inventory.perishables()
    if perishable:
        report["perishable"] = [lot.lot_id for lot in perishable]
    return report


def shipment_report(tracker):
    states = tracker.states()
    carriers = Counter()
    rows = []
    for s in tracker.shipments.values():
        rows.append(pick(s.to_dict(), ["id", "state", "carrier", "boxes"]))
        if s.carrier is not None:
            carriers[s.carrier.code] += 1
    return {"states": states, "rows": rows, "carriers": dict(carriers), "n": len(rows)}


def bus_report(bus):
    by_topic = defaultdict(int)
    for e in bus.log:
        by_topic[e["topic"]] += 1
    return {"events": len(bus.log), "by_topic": dict(by_topic), "topics": sorted(by_topic)}


@plugin("summary")
def p_summary(report, ctx):
    report["summary"] = {"lots": report["stock"]["lots"], "shipments": report["shipments"]["n"], "events": report["bus"]["events"]}


@plugin("site", after=("summary",))
def p_site(report, ctx):
    report["summary"]["site"] = ctx["cfg"].get("site.name")
    report["summary"]["profile"] = ctx["cfg"].profile


@plugin("audit", after=("summary",))
def p_audit(report, ctx):
    if ctx["cfg"].feature("audit"):
        report["audit"] = {"entries": len(ctx["sink"]), "topics": sorted({e["topic"] for e in ctx["sink"]})}


@plugin("shortages")
def p_shortages(report, ctx):
    if ctx["planner"].shortages:
        report["shortages"] = list(ctx["planner"].shortages)


def run_plugins(report, ctx, names):
    done = []
    for name in names:
        fn, after = PLUGINS[name]
        for dep in after:
            if dep not in done:
                raise RuntimeError("plugin " + name + " needs " + dep)
        fn(report, ctx)
        done.append(name)
    return done


def full_report(inventory, tracker, bus, planner, cfg, sink, plugins=("summary", "site", "audit", "shortages")):
    report = {"stock": stock_report(inventory), "shipments": shipment_report(tracker), "bus": bus_report(bus)}
    ctx = {"cfg": cfg, "sink": sink, "planner": planner}
    report["plugins"] = run_plugins(report, ctx, list(plugins))
    return report


def round_trip(report):
    return json.loads(json.dumps(report))


def csv_rows(report):
    rows = [["sku", "qty"]]
    for sku, qty in sorted(report["stock"]["skus"].items()):
        rows.append([sku, str(qty)])
    return rows
