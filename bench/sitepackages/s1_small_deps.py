"""Scenario 1: a script on a few small pure-Python dependencies (dateutil, tabulate,
humanize; attrs is modelled, so never followed)."""

import sys

import attrs
import humanize
from dateutil import parser as dateparser
from dateutil.relativedelta import relativedelta
from tabulate import tabulate


@attrs.define
class Release:
    name: str
    date: str
    downloads: int


RELEASES = [
    Release("1.0", "2024-03-01", 1_250_000),
    Release("1.1", "2024-09-15T10:30:00", 98_000),
    Release("2.0", "March 3, 2025", 4_200),
]


def age_rows(releases: list[Release], today: str) -> list[dict]:
    now = dateparser.parse(today)
    rows = []
    for r in releases:
        when = dateparser.parse(r.date)
        delta = relativedelta(now, when)
        rows.append({
            "release": r.name,
            "date": when.date().isoformat(),
            "age": f"{delta.years}y {delta.months}m",
            "downloads": humanize.intcomma(r.downloads),
            "size": humanize.naturalsize(r.downloads * 1024),
        })
    return rows


def next_review(date_text: str) -> str:
    d = dateparser.parse(date_text) + relativedelta(months=+3, day=1)
    return d.strftime("%Y-%m-%d")


def main() -> int:
    rows = age_rows(RELEASES, "2025-10-01")
    first = rows[0]
    table = tabulate(rows, headers="keys", tablefmt="github")
    print(table)
    print("next review:", next_review("2025-10-17"), first["age"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
