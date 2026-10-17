"""Scenario 2: the nightly invoice run, using the company's own billing package
(`acme_billing`, installed into the venv's site-packages like any dependency) beside two
small public ones (dateutil, tabulate)."""

import json
import sys

from dateutil import parser as dateparser
from tabulate import tabulate

from acme_billing import (
    ValidationError, build_invoice, customer_from_dict, invoice_to_dict, load_settings,
    render_rows, report_rows, summarise_by, top_products, validate_order,
)

CUSTOMERS = [
    {"id": "C-1", "name": "Northwind GmbH", "region": "eu", "vat_id": "DE123", "spend": 61_000.0},
    {"id": "C-2", "name": "Bob's Bikes", "region": "uk", "spend": 2_400.0},
    {"id": "C-3", "name": "Initech", "region": "us", "spend": 12_500.0, "tags": ["Partner"]},
]

ORDERS = [
    {"order_id": "O-100", "customer": "C-1", "lines": [{"sku": "A100", "qty": 600}, {"sku": "C001", "qty": 4}]},
    {"order_id": "O-101", "customer": "C-2", "currency": "GBP", "coupon": "WELCOME10",
     "lines": [{"sku": "B310", "qty": 2}]},
    {"order_id": "O-102", "customer": "C-3", "currency": "USD", "lines": [{"sku": "B320", "qty": 11}]},
    {"order_id": "O-103", "customer": "C-2", "lines": [{"sku": "Z999", "qty": 1}]},
]


def run(settings: dict) -> tuple[list[dict], list[dict]]:
    customers = {c["id"]: customer_from_dict(c) for c in CUSTOMERS}
    invoices, rejected = [], []
    for order in ORDERS:
        problems = validate_order(order, settings)
        if problems:
            rejected.append({"order_id": order["order_id"], "problems": problems})
            continue
        invoice = build_invoice(order, customers[order["customer"]], settings)
        invoices.append(invoice_to_dict(invoice))
    return invoices, rejected


def handle_webhook(payload: dict) -> dict:
    """Not called here: the order service calls it with a decoded request body."""
    settings = load_settings()
    customer = customer_from_dict(payload["customer"])
    try:
        invoice = invoice_to_dict(build_invoice(payload["order"], customer, settings))
    except ValidationError as exc:
        return {"ok": False, "error": exc.to_dict()}
    return {"ok": True, "number": invoice["number"], "total": invoice["total"]}


def main(argv: list[str]) -> int:
    run_date = dateparser.parse(argv[0] if argv else "2026-01-01").date()
    settings = load_settings({"payment_terms_days": 14})
    invoices, rejected = run(settings)
    rows = report_rows(invoices)
    by_region = summarise_by(rows, "region")
    eu_total = by_region["eu"]["total"]
    best = top_products(invoices, n=2)
    first = invoices[0]
    due = first["due"]
    print(f"invoice run of {run_date.isoformat()}")
    print(render_rows(rows))
    print(tabulate([[region, s["invoices"], s["total"]] for region, s in by_region.items()], headers=["region", "invoices", "total"]))
    print(json.dumps({"eu_total": eu_total, "best": best, "due": due, "rejected": rejected}))
    for r in rejected:
        print(r["order_id"], r["problems"][0]["message"])
    return 1 if rejected else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
