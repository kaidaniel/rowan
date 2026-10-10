"""Text output: Template headers, dates, money, fixed-width tables, wrapped notes."""
import textwrap
from datetime import datetime
from string import Template

HEADER = Template("$title ($currency) - $period")
SUMMARY = Template("$region: $orders orders / $qty units")


def period_label(dates):
    ds = sorted(datetime.strptime(d, "%Y-%m-%d") for d in dates)
    return f"{ds[0].strftime('%d %b')} to {ds[-1].strftime('%d %b %Y')}"


def header(cfg, period):
    return HEADER.substitute(title=cfg["title"], currency=cfg["currency"], period=period)


def money(x, currency):
    return f"{x:,.2f} {currency}"


def table(rows, key, cfg):
    lines = [f"{r[key]:<12}{r['qty']:>6}{money(r['revenue'], cfg['currency']):>16}" for r in rows]
    return "\n".join(lines)


def summary_line(rec):
    return SUMMARY.safe_substitute(rec)


def wrap_note(text, width):
    return textwrap.wrap(text, width=width)
