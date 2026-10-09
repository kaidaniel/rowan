"""A warehouse system as a package of twelve modules (`wh/`), ~1,900 lines with this entry:
layered configuration with profiles, dataclass entities (one dataclass subclassing another,
`__post_init__`, `classmethod` constructors), an inventory with FIFO lot consumption and
function-keyed indexes, inbound receipts validated by a rule chain, an order book with a
repository and secondary indexes, pick planning with decorator-registered strategies and
box packing, shipping with rate rules, a guarded tracking state machine and hooks, an event
bus with closures / bound methods / wildcard handlers, reports with a plugin chain and a
JSON round trip, a scheduler with generators and a context manager, labels, and cycle
counts.  Three scenarios run the same wiring with different profiles and inputs.

Expectations were written from the program text before running the engine.  Comparisons
between known numbers (`10 <= 5`, `0 <= 0`, `60000 >= 50000`) and arithmetic on known
numbers are treated as decided by the program; the JSON round trip is open by design."""
import wh
from wh import Location, Perishable, Product, build_config, order_record
from wh.config import validate
from wh.events import wire_bus
from wh.inventory import Inventory
from wh.labels import code_for, location_label, lot_label, parse_labels, shipment_label
from wh.maintenance import cycle_count, health
from wh.orders import Cart, OrderBook, Repo, seed_customers
from wh.picking import Planner
from wh.receiving import ReceiptProcessor, manifest_rows
from wh.reports import csv_rows, full_report, round_trip
from wh.scheduling import plan_day, task_counts
from wh.shipping import CARRIERS, Tracker, count_events, pick_carrier, rate_for, register_carrier, stamp_meta
from wh.util import deep_get, flatten

register_carrier("dhl", "DHL", ["A", "B"])
register_carrier("ups", "UPS", ["C"])


def build_world(profile, overrides, sink):
    cfg = build_config(profile, overrides)
    wired = wire_bus(cfg, sink)
    inv = Inventory(cfg, wired["bus"])
    inv.add_product(Product("a", "Apple Box", "box", 2000, ["fruit"]))
    inv.add_product(Perishable("m", "Milk", "ea", 1000, [], {"cold": True}, 7))
    inv.add_product(Product("t", "Tool", "ea", 5000))
    inv.add_location(Location("A-01", "A", 100))
    inv.add_location(Location("A-02", "A", 50, {"cold": True}))
    inv.add_location(Location("B-01", "B"))
    customers = seed_customers(Repo("customers", lambda c: c.cid, {"tier": lambda c: c.tier}))
    return {"cfg": cfg, "bus": wired["bus"], "tally": wired["tally"], "inv": inv, "customers": customers, "sink": sink}


def run_scenario(profile, overrides, manifests, carts, extra_orders=()):
    sink = []
    w = build_world(profile, overrides, sink)
    cfg, inv, bus = w["cfg"], w["inv"], w["bus"]
    proc = ReceiptProcessor(inv, cfg)
    receipts = {}
    for ref, spec in manifests:
        receipts[ref] = proc.process(ref, manifest_rows(spec))
    book = OrderBook(inv, w["customers"])
    placed = []
    for cid, items, priority, notes in carts:
        cart = Cart(w["customers"].get(cid))
        for sku, qty in items:
            cart.add(sku, qty)
        for k, v in notes.items():
            cart.note(k, v)
        order = book.place(cart, priority)
        if order is not None:
            placed.append(order)
    planner = Planner(inv, cfg)
    wave = planner.plan_wave(book.open_orders())
    extra_plans = [planner.plan_order(o) for o in extra_orders]
    tracker = Tracker(bus)
    tracker.hook("after", stamp_meta)
    hook_counts = {}
    tracker.hook("before", count_events(hook_counts))
    shipments = []
    rates = []
    for plan in wave["plans"]:
        order = book.orders.get(plan["order"])
        zone = order["customer"]["address"]["zone"]
        carrier = pick_carrier(cfg, zone) if order.get("rush") else None
        s = tracker.create(order, carrier)
        tracker.pack(s, planner.pack(plan))
        book.set_status(plan["order"], "picked")
        rate = rate_for(cfg, s, 60000 if order.get("rush") else 1000)
        rates.append(rate)
        tracker.label(s, rate)
        tracker.fire(s, "ship")
        if cfg.feature("audit"):
            tracker.fire(s, "deliver")
            tracker.fire(s, "pack")
        shipments.append(s)
    cfg.get("site.region", "eu")
    report = full_report(inv, tracker, bus, planner, cfg, sink)
    sched = plan_day(cfg, sorted(receipts), placed)
    digest = sched.run({"receive": lambda t: {"ref": t.ref, "ok": True}, "pick": lambda t: {"order": t.ref}})
    return {
        "cfg": cfg,
        "bus": bus,
        "tally": w["tally"],
        "inv": inv,
        "proc": proc,
        "receipts": receipts,
        "book": book,
        "placed": placed,
        "planner": planner,
        "wave": wave,
        "extra_plans": extra_plans,
        "tracker": tracker,
        "shipments": shipments,
        "rates": rates,
        "hook_counts": hook_counts,
        "report": report,
        "sched": sched,
        "digest": digest,
        "sink": sink,
    }


MANIFEST_1 = {
    "items": [("a", 10, "A-01"), ("m", 5, "A-02"), ("t", 2, "B-01"), ("x", 1, "A-01"), ("a", 0, "A-01")],
    "extras": {"a": {"lot_ref": "LA"}, "m": {"lot_ref": "LM", "expires": 20, "temp_ok": True}, "t": {"lot_ref": "LT"}, "x": {"lot_ref": "LX"}},
}
MANIFEST_2 = {"items": [("t", 3, "B-01")], "extras": {}}
MANIFEST_COLD = {"items": [("a", 10, "A-01"), ("m", 5, "A-02")], "extras": {"a": {"lot_ref": "LA"}, "m": {"lot_ref": "LM", "expires": 20}}}
MANIFEST_SMALL = {"items": [("a", 3, "A-01"), ("t", 1, "B-01")], "extras": {"a": {"lot_ref": "LA"}, "t": {"lot_ref": "LT"}}}

S1 = run_scenario(
    None,
    {"features": {"audit": True}},
    [("R1", MANIFEST_1), ("R2", MANIFEST_2)],
    [(1, [("a", 4), ("m", 2), ("a", 1)], "rush", {"gift": True}), (2, [("t", 1)], "normal", {}), (3, [("t", 5)], "normal", {}), (3, [], "normal", {})],
)
S2 = run_scenario(
    "cold",
    {"picking": {"strategy": "zone"}},
    [("R1", MANIFEST_COLD)],
    [(1, [("a", 4)], "rush", {}), (2, [("m", 1)], "normal", {})],
)
S3 = run_scenario(
    "small",
    {},
    [("R1", MANIFEST_SMALL)],
    [(1, [("a", 2)], "rush", {}), (2, [("t", 1)], "normal", {}), (3, [("a", 1)], "normal", {})],
    extra_orders=[order_record(104, {"id": 3, "name": "Cy", "tier": "silver", "address": {"zone": "C"}}, [{"sku": "a", "qty": 2}])],
)

# ---------------------------------------------------------------- configuration

cfg1 = S1["cfg"]                                   #@row cfg1: present={data,profile,reads,misses} definite={data,profile,reads,misses} closed
cfg1_data = cfg1.data                              #@row cfg1_data: present={site,picking,shipping,receiving,features,limits} definite={site,picking,shipping,receiving,features,limits} closed
cfg1_features = cfg1_data["features"]              #@row cfg1_features: present={audit,fast_track,cold_chain} definite={audit,fast_track,cold_chain} closed
cfg2_features = S2["cfg"].data["features"]         #@row cfg2_features: present={audit,fast_track,cold_chain} definite={audit,fast_track,cold_chain} closed
cfg3_limits = S3["cfg"].data["limits"]             #@row cfg3_limits: present={max_boxes,box_weight_g} definite={max_boxes,box_weight_g} closed
misses = cfg1.misses                               #@row misses: present={site.region} definite={site.region} closed
described = cfg1.describe()                        #@row described: present={site,profile,reads,misses} definite={site,profile,reads,misses} closed
valid1 = validate(cfg1)                            #@row valid1: present={ok,problems,checked} definite={ok,problems,checked} closed
valid2 = validate(S2["cfg"])                       #@row valid2: present={ok,problems,checked} definite={ok,problems,checked} closed
flat = flatten(cfg1_data)                          #@row flat: present={site.name,site.timezone,picking.strategy,picking.wave_size,picking.zones,shipping.default_carrier,shipping.free_over_g,shipping.insure_over,receiving.require_lot,receiving.quarantine_days,features.audit,features.fast_track,features.cold_chain,limits.max_boxes,limits.box_weight_g} definite={site.name,site.timezone,picking.strategy,picking.wave_size,picking.zones,shipping.default_carrier,shipping.free_over_g,shipping.insure_over,receiving.require_lot,receiving.quarantine_days,features.audit,features.fast_track,features.cold_chain,limits.max_boxes,limits.box_weight_g} closed
section = cfg1.section("shipping")                 #@row section: present={default_carrier,free_over_g,insure_over} definite={default_carrier,free_over_g,insure_over} closed
no_section = cfg1.section("nope")                  #@row no_section: present={} definite={} closed
version = wh.VERSION                               #@row version: present={major,minor,tag} definite={major,minor,tag} closed

# ---------------------------------------------------------------- receiving and inventory

r1 = S1["receipts"]["R1"]                          #@row r1: present={ref,rows,accepted,rejected,lots} definite={ref,rows,accepted,rejected,lots} closed
r1_summary = r1.summary()                          #@row r1_summary: present={ref,rows,accepted,rejected,reasons} definite={ref,rows,accepted,rejected,reasons} closed
r2_summary = S1["receipts"]["R2"].summary()        #@row r2_summary: present={ref,rows,accepted,rejected,reasons} definite={ref,rows,accepted,rejected,reasons} closed
r1_rejected = r1.rejected[0]                       #@row r1_rejected: present={row,problem} definite={row,problem} closed
r1_problem = r1_rejected["problem"]                #@row r1_problem: present={rule,why,sku} definite={rule,why,sku} closed
r1_problem2 = r1.rejected[1]["problem"]            #@row r1_problem2: present={rule,why} definite={rule,why} closed
r2_problem = S1["receipts"]["R2"].rejected[0]["problem"]  #@row r2_problem: present={rule,why} definite={rule,why} closed
cold_problem = S2["receipts"]["R1"].rejected[0]["problem"]  #@row cold_problem: present={rule,why} definite={rule,why} closed
proc_stats = S1["proc"].stats()                    #@row proc_stats: present={receipts,lots,rejected} definite={receipts,lots,rejected} closed
inv1 = S1["inv"]                                   #@row inv1: present={cfg,bus,products,locations,lots,by_product,by_location,reservations,moves,seq} definite={cfg,bus,products,locations,lots,by_product,by_location,reservations,moves,seq} closed
lots1 = inv1.lots                                  #@row lots1: present={L001,L002,L003} definite={L001,L002,L003} closed
lot_m = lots1["L002"]                              #@row lot_m: present={lot_id,product,qty,location,expires,meta} definite={lot_id,product,qty,location,expires,meta} closed
lot_m_dict = lot_m.to_dict()                       #@row lot_m_dict: present={lot,sku,qty,loc,expires} definite={lot,sku,qty,loc,expires} closed
lot_a_dict = lots1["L001"].to_dict()               #@row lot_a_dict: present={lot,sku,qty,loc} definite={lot,sku,qty,loc} closed
lot_meta = lot_m.meta                              #@row lot_meta: present={ref,lot_ref} definite={ref,lot_ref} closed
milk = lot_m.product                               #@row milk: present={sku,name,unit,weight_g,tags,attrs,shelf_days} definite={sku,name,unit,weight_g,tags,attrs,shelf_days} closed
milk_dict = milk.to_dict()                         #@row milk_dict: present={sku,name,unit,weight_g,attrs,shelf_days} definite={sku,name,unit,weight_g,attrs,shelf_days} closed
apple_dict = inv1.products["a"].to_dict()          #@row apple_dict: present={sku,name,unit,weight_g,tags} definite={sku,name,unit,weight_g,tags} closed
tool_dict = inv1.products["t"].to_dict()           #@row tool_dict: present={sku,name,unit,weight_g} definite={sku,name,unit,weight_g} closed
loc_a2 = inv1.locations["A-02"]                    #@row loc_a2: present={code,zone,capacity,attrs,aisle} definite={code,zone,capacity,attrs,aisle} closed
loc_dict = loc_a2.to_dict()                        #@row loc_dict: present={code,zone,capacity,aisle} definite={code,zone,capacity,aisle} closed
by_product = inv1.by_product.groups                #@row by_product: present={a,m,t} definite={a,m,t} closed
by_location = inv1.by_location.groups              #@row by_location: present={A-01,A-02,B-01} definite={A-01,A-02,B-01} closed
index_summary = inv1.by_product.summary()          #@row index_summary: present={name,groups,n} definite={name,groups,n} closed
move0 = inv1.moves[0]                              #@row move0: present={op,lot,qty,to} definite={op,lot,qty,to} closed
move3 = inv1.moves[3]                              #@row move3: present={op,lot,qty,why} definite={op,lot,qty,why} closed
zones1 = inv1.count_by_zone()                      #@row zones1: present={A,B} definite={A,B} closed
located = inv1.locate("a")                         #@row located: present={A-01} definite={A-01} closed
snapshot = inv1.snapshot()                         #@row snapshot: present={products,lots,zones,moves,reserved} definite={products,lots,zones,moves,reserved} closed
snap_lots = snapshot["lots"]                       #@row snap_lots: present={L001,L002,L003} definite={L001,L002,L003} closed
reserved1 = inv1.reservations                      #@row reserved1: present={a,m,t} definite={a,m,t} closed

# ---------------------------------------------------------------- orders

book1 = S1["book"]                                 #@row book1: present={inventory,customers,orders,seq,rejected} definite={inventory,customers,orders,seq,rejected} closed
order101 = S1["placed"][0]                         #@row order101: present={id,customer,lines,priority,status,rush,notes,transitions} definite={id,customer,lines,priority,status,rush,notes,transitions} closed
order102 = S1["placed"][1]                         #@row order102: present={id,customer,lines,priority,status,transitions} definite={id,customer,lines,priority,status,transitions} closed
order101_customer = order101["customer"]           #@row order101_customer: present={id,name,tier,address} definite={id,name,tier,address} closed
order101_notes = order101["notes"]                 #@row order101_notes: present={gift} definite={gift} closed
line0 = order101["lines"][0]                       #@row line0: present={sku,qty} definite={sku,qty} closed
rejected0 = book1.rejected[0]                      #@row rejected0: present={customer,problems} definite={customer,problems} closed
stock_problem = rejected0["problems"][0]           #@row stock_problem: present={why,sku,available} definite={why,sku,available} closed
empty_problem = book1.rejected[1]["problems"][0]   #@row empty_problem: present={why} definite={why} closed
totals1 = book1.totals()                           #@row totals1: present={orders,rejected,lines} definite={orders,rejected,lines} closed
orders_repo = book1.orders.describe()              #@row orders_repo: present={name,n,indexes} definite={name,n,indexes} closed
status_index = book1.orders.indexes["status"]      #@row status_index: present={open} definite={open} closed
customer_index = book1.orders.indexes["customer"]  #@row customer_index: present={1,2} definite={1,2} closed
tier_index = book1.customers.indexes["tier"]      #@row tier_index: present={gold,basic,silver} definite={gold,basic,silver} closed
ann = book1.customers.get(1)                       #@row ann: present={cid,name,tier,address,orders} definite={cid,name,tier,address,orders} closed
ann_dict = ann.to_dict()                           #@row ann_dict: present={id,name,tier,address,orders} definite={id,name,tier,address,orders} closed
cy_dict = book1.customers.get(3).to_dict()         #@row cy_dict: present={id,name,tier,address} definite={id,name,tier,address} closed
repo_event = book1.orders.events[1]                #@row repo_event: present={op,key} definite={op,key} closed

# ---------------------------------------------------------------- picking

planner1 = S1["planner"]                           #@row planner1: present={inventory,cfg,walks,waves,shortages,strategy} definite={inventory,cfg,walks,waves,shortages,strategy} closed
wave1 = S1["wave"]                                 #@row wave1: present={n,plans,by_zone} definite={n,plans,by_zone} closed
plan101 = wave1["plans"][0]                        #@row plan101: present={order,picks,lines,rush} definite={order,picks,lines,rush} closed
plan102 = wave1["plans"][1]                        #@row plan102: present={order,picks,lines} definite={order,picks,lines} closed
pick0 = plan101["picks"][0]                        #@row pick0: present={sku,lot,qty,loc} definite={sku,lot,qty,loc} closed
by_zone1 = wave1["by_zone"]                        #@row by_zone1: present={A,B} definite={A,B} closed
planner_desc = planner1.describe()                 #@row planner_desc: present={strategy,waves,walks,shortages} definite={strategy,waves,walks,shortages} closed
strategy_desc = planner_desc["strategy"]           #@row strategy_desc: present={name} definite={name} closed
planner2_desc = S2["planner"].describe()["strategy"]  #@row planner2_desc: present={name} definite={name} closed
boxes101 = S1["shipments"][0].boxes
box1 = boxes101[1]                                 #@row box1: present={n,items,weight_g} definite={n,items,weight_g} closed
box0 = boxes101[0]                                 #@row box0: present={n,items,weight_g} definite={n,items,weight_g} closed
box_item = box0["items"][0]                        #@row box_item: present={sku,qty} definite={sku,qty} closed
shortage = S3["planner"].shortages[0]              #@row shortage: present={order,sku,short} definite={order,sku,short} closed
extra_plan = S3["extra_plans"][0]                  #@row extra_plan: present={order,picks,lines} definite={order,picks,lines} closed
# Order 102 (the tool) was not in the one-order wave, so its lot is still there.
consumed = S3["inv"].lots                          #@row consumed: present={L002} definite={L002} closed

# ---------------------------------------------------------------- shipping

s1 = S1["shipments"][0]                            #@row s1: present={ship_id,order,carrier,state,boxes,history,label,meta} definite={ship_id,order,carrier,state,boxes,history,label,meta} closed
s1_dict = s1.to_dict()                             #@row s1_dict: present={id,order,state,boxes,carrier,label} definite={id,order,state,boxes,carrier,label} closed
s1_label = s1.label                                #@row s1_label: present={carrier,to,cost,boxes,insured} definite={carrier,to,cost,boxes,insured} closed
s1_meta = s1.meta                                  #@row s1_meta: present={last_event,rejected} definite={last_event,rejected} closed
s1_hist = s1.history[3]                            #@row s1_hist: present={from,to,event} definite={from,to,event} closed
s2 = S1["shipments"][1]                            #@row s2: present={ship_id,order,carrier,state,boxes,history,label,meta} definite={ship_id,order,carrier,state,boxes,history,label,meta} closed
s2_dict = s2.to_dict()                             #@row s2_dict: present={id,order,state,boxes} definite={id,order,state,boxes} closed
# s2 was packed (last_event), its label was blocked by the carrier guard, and the "ship"
# event from the packed state was rejected.
s2_meta = s2.meta                                  #@row s2_meta: present={last_event,blocked,rejected} definite={last_event,blocked,rejected} closed
rate1 = S1["rates"][0]                             #@row rate1: present={base_cents,currency,discount_cents,free,insurance_cents,total_cents,total} definite={base_cents,currency,discount_cents,free,insurance_cents,total_cents,total} closed
rate2 = S1["rates"][1]                             #@row rate2: present={base_cents,currency,total_cents,total} definite={base_cents,currency,total_cents,total} closed
rate_s2 = S2["rates"][0]                           #@row rate_s2: present={base_cents,currency,discount_cents,free,insurance_cents,total_cents,total} definite={base_cents,currency,discount_cents,free,insurance_cents,total_cents,total} closed
label_s2 = S2["shipments"][0].label                #@row label_s2: present={carrier,to,cost,boxes,insured} definite={carrier,to,cost,boxes,insured} closed
hook_counts = S1["hook_counts"]                    #@row hook_counts: present={pack,label,ship,deliver} definite={pack,label,ship,deliver} closed
hook_counts_s2 = S2["hook_counts"]                 #@row hook_counts_s2: present={pack,label,ship} definite={pack,label,ship} closed
states1 = S1["tracker"].states()                   #@row states1: present={delivered,packed} definite={delivered,packed} closed
states2 = S2["tracker"].states()                   #@row states2: present={shipped} definite={shipped} closed
dhl = CARRIERS["dhl"]                              #@row dhl: present={code,name,zones,stats} definite={code,name,zones,stats} closed
dhl_desc = dhl.describe()                          #@row dhl_desc: present={code,name,zones} definite={code,name,zones} closed
carriers = CARRIERS                                #@row carriers: present={dhl,ups} definite={dhl,ups} closed

# ---------------------------------------------------------------- events

tally1 = S1["tally"]                               #@row tally1: present={by_topic,last} definite={by_topic,last} closed
# The tracker emits "shipment." + event, so the ship event's topic is "shipment.ship"; the
# handlers registered on "shipment.shipped" never run (a bug in the program, kept as is).
tally_topics = tally1.by_topic                     #@row tally_topics: present={lot.received,shipment.pack,shipment.label,shipment.ship,shipment.deliver} definite={lot.received,shipment.pack,shipment.label,shipment.ship,shipment.deliver} closed
received_event = S1["bus"].log[0]                  #@row received_event: present={topic,payload,handled,audited,tallied} definite={topic,payload,handled,audited,tallied} closed
received_payload = received_event["payload"]       #@row received_payload: present={lot,sku} definite={lot,sku} closed
pack_event = S1["bus"].log[3]                      #@row pack_event: present={topic,payload,handled,tallied} definite={topic,payload,handled,tallied} closed
shipped_event = S1["bus"].last("shipment.ship")    #@row shipped_event: present={topic,payload,handled,tallied} definite={topic,payload,handled,tallied} closed
shipped_payload = shipped_event["payload"]         #@row shipped_payload: present={shipment,event} definite={shipment,event} closed
shipped_s2 = S2["bus"].last("shipment.ship")       #@row shipped_s2: present={topic,payload,handled,tallied} definite={topic,payload,handled,tallied} closed
never = S1["bus"].last("shipment.shipped")
never_row = {"none": never}                        #@row never_row: present={none} definite={none} closed
bus_counts = S1["bus"].counts                      #@row bus_counts: present={lot.received,shipment.pack,shipment.label,shipment.ship,shipment.deliver} definite={lot.received,shipment.pack,shipment.label,shipment.ship,shipment.deliver} closed
audit0 = S1["sink"][0]                             #@row audit0: present={topic,keys} definite={topic,keys} closed
no_audit = S2["sink"]

# ---------------------------------------------------------------- reports

report1 = S1["report"]                             #@row report1: present={stock,shipments,bus,summary,audit,plugins} definite={stock,shipments,bus,summary,audit,plugins} closed
stock1 = report1["stock"]                          #@row stock1: present={zones,skus,top,lots,reserved,perishable} definite={zones,skus,top,lots,reserved,perishable} closed
stock_skus = stock1["skus"]                        #@row stock_skus: present={a,m,t} definite={a,m,t} closed
top0 = stock1["top"][0]                            #@row top0: present={sku,qty} definite={sku,qty} closed
ship_report = report1["shipments"]                 #@row ship_report: present={states,rows,carriers,n} definite={states,rows,carriers,n} closed
ship_row0 = ship_report["rows"][0]                 #@row ship_row0: present={id,state,carrier,boxes} definite={id,state,carrier,boxes} closed
ship_row1 = ship_report["rows"][1]                 #@row ship_row1: present={id,state,boxes} definite={id,state,boxes} closed
ship_carriers = ship_report["carriers"]            #@row ship_carriers: present={dhl} definite={dhl} closed
bus1 = report1["bus"]                              #@row bus1: present={events,by_topic,topics} definite={events,by_topic,topics} closed
summary1 = report1["summary"]                      #@row summary1: present={lots,shipments,events,site,profile} definite={lots,shipments,events,site,profile} closed
audit_section = report1["audit"]                   #@row audit_section: present={entries,topics} definite={entries,topics} closed
report2 = S2["report"]                             #@row report2: present={stock,shipments,bus,summary,plugins} definite={stock,shipments,bus,summary,plugins} closed
report3 = S3["report"]                             #@row report3: present={stock,shipments,bus,summary,plugins,shortages} definite={stock,shipments,bus,summary,plugins,shortages} closed
stock3 = report3["stock"]                          #@row stock3: present={zones,skus,top,lots,reserved} definite={zones,skus,top,lots,reserved} closed
rt = round_trip(report1)                           #@row rt: present={} definite={} open
csv = csv_rows(report1)

# ---------------------------------------------------------------- scheduling

digest1 = S1["digest"]                             #@row digest1: present={tasks,by_state,by_slot,done,timed} definite={tasks,by_state,by_slot,done,timed} closed
by_state = digest1["by_state"]                     #@row by_state: present={done,skipped} definite={done,skipped} closed
by_slot = digest1["by_slot"]                       #@row by_slot: present={morning,midday,afternoon} definite={morning,midday,afternoon} closed
timings = S1["sched"].timings                      #@row timings: present={pick:101,pick:102,receive:R1,receive:R2} definite={pick:101,pick:102,receive:R1,receive:R2} closed
timing0 = timings["pick:101"]                      #@row timing0: present={started,done} definite={started,done} closed
task_dict = S1["sched"].tasks[2].to_dict()         #@row task_dict: present={kind,ref,priority,state,slot,result} definite={kind,ref,priority,state,slot,result} closed
ship_task = S1["sched"].tasks[3].to_dict()         #@row ship_task: present={kind,ref,priority,state,slot} definite={kind,ref,priority,state,slot} closed
kinds = task_counts(S1["sched"])                   #@row kinds: present={receive,pick,ship} definite={receive,pick,ship} closed

# ---------------------------------------------------------------- labels and maintenance

milk_label = lot_label(lot_m)                      #@row milk_label: present={text,key,sku,expires} definite={text,key,sku,expires} closed
apple_label = lot_label(lots1["L001"])             #@row apple_label: present={text,key,sku} definite={text,key,sku} closed
s1_label_full = shipment_label(s1)                 #@row s1_label_full: present={carrier,to,cost,boxes,insured,key,state} definite={carrier,to,cost,boxes,insured,key,state} closed
s2_label_short = shipment_label(s2)                #@row s2_label_short: present={key,state} definite={key,state} closed
loc_label = location_label(loc_a2)                 #@row loc_label: present={text,key,zone} definite={text,key,zone} closed
code = code_for(order102)                          #@row code: present={code,n} definite={code,n} closed
parsed = parse_labels([apple_label, s2_label_short])  #@row parsed: present={L001,S02} definite={L001,S02} closed
count_a = cycle_count(inv1, "A-01", {"L001": 5, "L009": 1})  #@row count_a: present={loc,counted,diffs,applied,log} definite={loc,counted,diffs,applied,log} closed
count_log = count_a["log"][0]                      #@row count_log: present={lot,skipped} definite={lot,skipped} closed
count_b = cycle_count(inv1, "B-01", {"L003": 0})   #@row count_b: present={loc,counted,diffs,applied,log} definite={loc,counted,diffs,applied,log} closed
applied0 = count_b["applied"][0]                   #@row applied0: present={lot,delta,emptied} definite={lot,delta,emptied} closed
health1 = health(inv1)                             #@row health1: present={lots,empty,moves,needs_cleanup} definite={lots,empty,moves,needs_cleanup} closed
health3 = health(S3["inv"])                        #@row health3: present={lots,empty,moves} definite={lots,empty,moves} closed

# ---------------------------------------------------------------- spot checks

order102["rush"]                                   #@ missing rush
order102["notes"]                                  #@ missing notes
rate2["free"]                                      #@ missing free
s2_dict["label"]                                   #@ missing label
report2["audit"]                                   #@ missing audit
report1["shortages"]                               #@ missing shortages
stock3["perishable"]                               #@ missing perishable
valid1["warn"]                                     #@ missing warn
s1_label["insured"]                                #@ ok insured
label_s2["insured"]                                #@ ok insured
health3["needs_cleanup"]                           #@ missing needs_cleanup
cfg1_features["audit"]                             #@ ok audit
rt["summary"]                                      #@ input summary
