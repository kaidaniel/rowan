"""The command line: `report regions|categories|reps [--region R]… [--top N] [--min-qty N] [--json]`."""
import argparse
import json

from reporting.config import apply_env, load_config
from reporting.fmt import header, period_label, table
from reporting.loader import SALES_CSV, load_products, load_sales
from reporting.transform import by_category, region_records, sales_frame, top_reps

KEYS = {"regions": "region", "categories": "category", "reps": "rep"}


def build_parser():
    p = argparse.ArgumentParser(prog="report")
    p.add_argument("command", choices=sorted(KEYS))
    p.add_argument("--region", action="append", dest="regions")
    p.add_argument("--top", type=int, default=2)
    p.add_argument("--min-qty", type=int, dest="min_qty")
    p.add_argument("--json", action="store_true")
    return p


def run(argv, csv_text=SALES_CSV, ini=None):
    args = build_parser().parse_args(argv)
    cfg = apply_env(load_config() if ini is None else load_config(ini))
    min_qty = args.min_qty if args.min_qty is not None else cfg["min_qty"]
    sales = load_sales(csv_text, min_qty)
    df = sales_frame(sales, load_products())
    result = {"command": args.command, "header": header(cfg, period_label([s["date"] for s in sales]))}
    if args.command == "regions":
        result["rows"] = region_records(df, args.regions or cfg["regions"])
    elif args.command == "categories":
        result["rows"] = by_category(df).to_dict("records")
    else:
        result["rows"] = top_reps(df, args.top)
    if args.json:
        result["text"] = json.dumps(result["rows"])
    else:
        result["text"] = table(result["rows"], KEYS[args.command], cfg)
    result["rule"] = "=" * cfg["width"]
    if "footer" in cfg:
        result["footer"] = cfg["footer"]
    return result
