#@engine rust
"""A small command-line tool: argparse, csv rows, a json config, an output summary."""
import argparse
import csv
import json


def parse_args():
    p = argparse.ArgumentParser()
    p.add_argument("--input", required=True)
    p.add_argument("--config", default="cfg.json")
    p.add_argument("--dry-run", action="store_true")
    return p.parse_args()


def read_rows(path):
    with open(path) as f:
        return list(csv.DictReader(f))


def summarise(rows, cfg):
    by_region = {}
    for row in rows:
        region = row["region"]                   #@ input region
        entry = by_region.setdefault(region, {"n": 0, "total": 0.0})
        entry["n"] += 1
        entry["total"] += float(row["amount"])   #@ input amount
        if cfg["verbose"]:                       #@ input verbose
            entry["seen_ids"] = entry.get("seen_ids", []) + [row["id"]]   #@ input id
    return by_region


def main():
    args = parse_args()
    with open(args.config) as f:
        cfg = json.load(f)
    rows = read_rows(args.input)                 #@ ok input
    summary = summarise(rows, cfg)
    for region, entry in summary.items():
        entry["n"]                               #@ ok n
        entry["avg"] = entry["total"] / entry["n"]   #@ ok total
        entry["seen_ids"]                        #@ possibly seen_ids
    if args.dry_run:                             #@ ok dry_run
        print(summary)
    args.verbose                                 #@ missing verbose
    return summary


main()
#@schema csv.DictReader@L18: always={region,amount} sometimes={id}
#@schema json.load@L36: always={verbose}
