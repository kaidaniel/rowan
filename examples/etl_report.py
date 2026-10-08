"""A realistic-ish ETL script with the usual pitfalls, used as a smoke test (not a golden case)."""
import argparse
import csv
import json
import logging
import os
from collections import defaultdict
from dataclasses import dataclass, asdict

import pandas as pd

log = logging.getLogger(__name__)

DEFAULT_CONFIG = {"min_amount": 10, "regions": ["EU", "US"], "output": "report.csv"}


@dataclass
class Summary:
    region: str
    total: float
    n_orders: int


def load_config(path):
    cfg = dict(DEFAULT_CONFIG)
    if os.path.exists(path):
        with open(path) as f:
            cfg.update(json.load(f))
    cfg["threshold"] = cfg["min_amount"] * 2
    return cfg


def read_customers(path):
    customers = {}
    with open(path) as f:
        for row in csv.DictReader(f):
            customers[row["customer_id"]] = {
                "name": row["name"].strip(),
                "region": row.get("region", "EU"),
                "tier": row["tier"],
            }
    return customers


def enrich(orders: pd.DataFrame, customers):
    orders = orders.rename(columns={"cust": "customer_id", "amt": "amount"})
    orders["amount"] = orders["amount"].astype(float)
    orders["region"] = orders["customer_id"].map(lambda c: customers.get(c, {}).get("region"))
    orders = orders.dropna(subset=["region"])
    orders["big"] = orders["amount"] > 100
    return orders


def summarise(orders, cfg):
    kept = orders[orders["amount"] >= cfg["min_amount"]]
    by_region = kept.groupby("region").agg(total=("amount", "sum"), n_orders=("order_id", "count"))
    by_region = by_region.reset_index()
    out = []
    for _, r in by_region.iterrows():
        out.append(Summary(region=r["region"], total=r["total"], n_orders=r["n_orders"]))
        if r["totl"] > cfg["threshold"]:          # typo: totl
            log.info("big region %s", r["region"])
    return out

def f():
    x = {
        "one": 1,
        "two": 2,
    }
    return {"x": x}









def g():
    x = f()["x"]
    return x


def to_rows(summaries):
    rows = [asdict(s) for s in summaries]
    for row in rows:
        row["avg"] = row["total"] / row["n_orders"]
        row["label"] = f"{row['region']} ({row['count']})"   # bug: no 'count' key
    return rows


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--orders", required=True)
    p.add_argument("--customers", required=True)
    p.add_argument("--config", default="config.json")
    args = p.parse_args()

    cfg = load_config(args.config)
    customers = read_customers(args.customers)
    orders = pd.read_csv(args.orders)
    orders = enrich(orders, customers)
    summaries = summarise(orders, cfg)
    rows = to_rows(summaries)
    pd.DataFrame(rows).to_csv(cfg["output"], index=False)
    print(cfg["verbose"])                                   # not in defaults; only via config file
    counts = defaultdict(int)
    for s in summaries:
        counts[s.region] += 1
    print(counts["EU"], args.cusomers)                      # typo: cusomers


if __name__ == "__main__":
    main()
