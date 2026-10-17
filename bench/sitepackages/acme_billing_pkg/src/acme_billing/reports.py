"""Report rows for the finance dashboards and the monthly CSV export."""

from .money import format_money

REPORT_COLUMNS = ("number", "customer_id", "region", "currency", "lines", "subtotal", "tax", "total")


def report_rows(invoices: list[dict]) -> list[dict]:
    """One flat row per invoice (`invoice_to_dict`'s output), the columns of REPORT_COLUMNS."""
    rows = []
    for inv in invoices:
        rows.append({
            "number": inv["number"],
            "customer_id": inv["customer_id"],
            "region": inv["region"],
            "currency": inv["currency"],
            "lines": len(inv["lines"]),
            "subtotal": inv["subtotal"],
            "tax": inv["tax"],
            "total": inv["total"],
        })
    return rows


def summarise_by(rows: list[dict], key: str) -> dict[str, dict]:
    """Totals per value of `key` ("region", "customer_id", "currency")."""
    out: dict[str, dict] = {}
    for row in rows:
        group = row[key]
        acc = out.setdefault(group, {"invoices": 0, "subtotal": 0.0, "tax": 0.0, "total": 0.0})
        acc["invoices"] += 1
        acc["subtotal"] = round(acc["subtotal"] + row["subtotal"], 2)
        acc["tax"] = round(acc["tax"] + row["tax"], 2)
        acc["total"] = round(acc["total"] + row["total"], 2)
    return out


def top_products(invoices: list[dict], n: int = 3) -> list[tuple[str, int]]:
    """The `n` SKUs sold most (by quantity) over the invoices."""
    counts: dict[str, int] = {}
    for inv in invoices:
        for line in inv["lines"]:
            counts[line["sku"]] = counts.get(line["sku"], 0) + line["qty"]
    ranked = sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))
    return ranked[:n]


def render_rows(rows: list[dict], columns: tuple[str, ...] = REPORT_COLUMNS) -> str:
    """A plain-text table of rows (money columns formatted in each row's currency)."""
    money = {"subtotal", "tax", "total"}
    cells = [list(columns)]
    for row in rows:
        cells.append([format_money(row[c], row["currency"]) if c in money else str(row[c]) for c in columns])
    widths = [max(len(r[i]) for r in cells) for i in range(len(columns))]
    lines = ["  ".join(cell.ljust(w) for cell, w in zip(r, widths)) for r in cells]
    lines.insert(1, "  ".join("-" * w for w in widths))
    return "\n".join(lines)
