"""Model: a command-line tool turning an orders CSV into a grouped report (JSON or CSV), with an
optional JSON config file. argparse (dest, defaults, store_true, choices, explicit argv lists),
json.load of a file (input) vs json round trips of built records, csv.DictReader (open rows) /
DictWriter (fieldnames, extrasaction), defaultdict / Counter, records passed between helpers."""
import argparse
import csv
import io
import json
from collections import Counter, defaultdict

DEFAULTS = {"currency": "EUR", "tax": 0.2, "round": 2}
COLUMNS = ["group", "orders", "qty", "revenue"]


def build_parser():
    p = argparse.ArgumentParser(prog="orders")
    p.add_argument("source")
    p.add_argument("--config", default=None)
    p.add_argument("--format", choices=["json", "csv"], default="json")
    p.add_argument("--min-qty", type=int, default=0, dest="min_qty")
    p.add_argument("--group-by", default="customer", dest="group_by")
    p.add_argument("-q", "--quiet", action="store_true")
    return p


def load_config(path):
    if path is None:
        cfg = dict(DEFAULTS)
        _ = 0                                    #@row cfg: present={currency,tax,round} definite={currency,tax,round} closed
        return cfg
    with open(path) as f:
        user = json.load(f)
    if user["version"] != 1:                     #@ input version
        raise ValueError("unsupported config version")
    known = {k: v for k, v in user.items() if k in DEFAULTS}
    cfg = DEFAULTS | known
    _ = 0                                        #@row cfg: present={currency,tax,round} definite={currency,tax,round} closed
    cfg["round"]                                 #@ ok round
    extra = sorted(set(user) - set(DEFAULTS) - {"version"})
    if extra:
        cfg["ignored"] = extra
    return cfg
#@schema json.load@L32: always={version} only={version}


def normalize(row):
    rec = {
        "id": row["order_id"],
        "customer": row["customer"].strip(),
        "sku": row["sku"],
        "qty": int(row["qty"]),
        "price": float(row["price"]),
    }
    if row.get("coupon"):                        #@ none coupon
        rec["coupon"] = row["coupon"]            #@ ok coupon
    _ = 0                                        #@row rec: present={id,customer,sku,qty,price,coupon} definite={id,customer,sku,qty,price} closed
    return rec
#@schema param normalize.row: always={order_id,customer,sku,qty,price} only={order_id,customer,sku,qty,price}


def read_orders(path, min_qty):
    out = []
    with open(path, newline="") as f:
        for row in csv.DictReader(f):
            row["channel"]                       #@ input channel
            rec = normalize(row)
            if rec["qty"] >= min_qty:            #@ ok qty
                out.append(rec)
    return out


def summarize(orders, key, cfg):
    groups = defaultdict(list)
    for o in orders:
        groups[o[key]].append(o)                 #@ ok customer ; ok sku
    table = {}
    for name, items in groups.items():
        entry = {
            "orders": len(items),
            "qty": sum(i["qty"] for i in items),
            "revenue": round(sum(i["qty"] * i["price"] for i in items) * (1 + cfg["tax"]), cfg["round"]),
        }
        coupons = Counter(i["coupon"] for i in items if "coupon" in i)
        if coupons:
            entry["coupons"] = dict(coupons)
        _ = 0                                    #@row entry: present={orders,qty,revenue,coupons} definite={orders,qty,revenue} closed
        table[name] = entry
    return table


def top_group(table):
    if not table:
        return {"group": None, "revenue": 0}
    name = max(table, key=lambda k: table[k]["revenue"])
    return {"group": name, "revenue": table[name]["revenue"]}


def to_rows(table):
    rows = []
    for name, entry in sorted(table.items()):
        row = {"group": name, **entry}
        _ = 0                                    #@row row: present={group,orders,qty,revenue,coupons} definite={group,orders,qty,revenue} closed
        rows.append(row)
    return rows


def render_csv(rows):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=COLUMNS, extrasaction="ignore", lineterminator="\n")
    w.writeheader()
    for r in rows:
        w.writerow(r)
    return buf.getvalue()


def strict_csv(rows):
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=COLUMNS, extrasaction="raise")
    try:
        for r in rows:
            w.writerow(r)
    except ValueError:
        return False
    return True


def render_json(meta, table, top):
    doc = {"meta": meta, "groups": table, "top": top}
    text = json.dumps(doc, sort_keys=True)
    back = json.loads(text)
    _ = 0                                        #@row back: present={meta,groups,top} definite={meta,groups,top} closed
    m = back["meta"]                             #@row m: present={currency,group_by,count,source} definite={currency,group_by,count,source} closed
    t = back["top"]                              #@row t: present={group,revenue} definite={group,revenue} closed
    return text


def run(argv):
    args = build_parser().parse_args(argv)
    _ = 0                                        #@row args: present={source,config,format,min_qty,group_by,quiet} definite={source,config,format,min_qty,group_by,quiet} closed
    try:
        args.min_qty                             #@ ok min_qty
        args.group                               #@ missing group
    except AttributeError:
        pass
    fmt = args.format                            #@value fmt: exactly={json,csv}
    cfg = load_config(args.config)
    cur = cfg["currency"]                        #@value cur: includes={EUR}
    orders = read_orders(args.source, args.min_qty)
    table = summarize(orders, args.group_by, cfg)
    top = top_group(table)
    meta = {"currency": cfg["currency"], "group_by": args.group_by, "count": len(orders), "source": args.source}
    if args.format == "csv":
        rows = to_rows(table)
        out = render_csv(rows)
        ok = strict_csv(rows)
        result = {"format": "csv", "body": out, "strict_ok": ok}
    else:
        out = render_json(meta, table, top)
        result = {"format": "json", "body": out}
    if not args.quiet:
        result["summary"] = f"{meta['count']} orders, top {top['group']}"
    _ = 0                                        #@row result: present={format,body,strict_ok,summary} definite={format,body} closed
    return result


def show(result):
    return result["summary"]                     #@ ok summary ; missing summary


if __name__ == "__main__":  # cur was exactly {EUR, USD} and ok2 {False}: both depend on the data files, run-time inputs
    r1 = run(["sm01_orders.csv", "--config", "sm01_config.json"])
    r2 = run(["sm01_orders.csv", "--group-by", "sku", "--format", "csv", "-q"])
    r3 = run(["--min-qty", "2", "sm01_orders.csv"])
    ok2 = r2["strict_ok"]                        #@value ok2: exactly={True,False}
    for r in (r1, r2, r3):
        try:
            show(r)
        except KeyError:
            pass
    empty = summarize([], "customer", DEFAULTS)  #@row empty: present={} closed
    none_top = top_group(empty)                  #@row none_top: present={group,revenue} definite={group,revenue} closed
    g = none_top["group"]                        #@value g: exactly={None}
