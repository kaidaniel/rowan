"""CLI: python -m rowan FILE.py [--json] [--no-rows]"""
from __future__ import annotations

import argparse
import json
import sys

from . import analyse_file
from .report import format_text, to_json


def main(argv=None) -> int:
    p = argparse.ArgumentParser(prog="rowan", description="static key-set inference for Python records")
    p.add_argument("file")
    p.add_argument("--json", action="store_true", help="emit JSON lines instead of text")
    p.add_argument("--no-rows", action="store_true", help="omit module-level rows")
    p.add_argument("--no-summaries", action="store_true", help="omit per-call function results")
    p.add_argument("--no-entry", action="store_true", help="do not analyse uncalled functions as entry points")
    p.add_argument("--no-imports", action="store_true", help="treat local imports as unknown modules")
    a = p.parse_args(argv)
    an = analyse_file(a.file, analyse_entry_points=not a.no_entry, follow_imports=not a.no_imports)
    if a.json:
        for rec in to_json(an):
            print(json.dumps(rec, default=str))
    else:
        print(format_text(an, show_rows=not a.no_rows, show_summaries=not a.no_summaries))
    return 0


if __name__ == "__main__":
    sys.exit(main())
