"""Turn bench/run.py results into github-action-benchmark's `customSmallerIsBetter` format.

Usage: python bench/history.py RESULTS_DIR OUT.json

One entry per program and metric ("rich: work", "rich: peak memory", …); metrics a run did
not produce (the work counters of a run cut by the timeout) are left out, and a cut run's
wall time is its timeout, marked in the entry's `extra` text.
"""

import json
import sys
from pathlib import Path

METRICS = [
    ("wall_s", "wall time", "s"),
    ("peak_rss_mb", "peak memory", "MB"),
    ("work", "work", "statements"),
    ("units", "work units", "units"),
    ("tabulations", "tabulations", "count"),
    ("objects", "objects", "count"),
]


def main() -> None:
    results, out = Path(sys.argv[1]), Path(sys.argv[2])
    entries = []
    for f in sorted(results.glob("*.json")):
        r = json.loads(f.read_text())
        if "golden_failing" in r:
            # the golden suites with every call past its budget (`tab=0,budget=0`): failing cases
            entries.append({"name": "golden tab=0: failing cases", "unit": "cases", "value": r["golden_failing"], "extra": f"{r['golden_passing']} passing"})
            continue
        if "name" not in r or "wall_s" not in r:
            continue
        note = "finished" if r.get("finished") else f"cut at the {r.get('timeout_s', 0):.0f} s timeout"
        for key, label, unit in METRICS:
            if r.get(key) is not None:
                entries.append({"name": f"{r['name']}: {label}", "unit": unit, "value": r[key], "extra": note})
    out.write_text(json.dumps(entries, indent=2))
    print(f"{len(entries)} entries")


if __name__ == "__main__":
    main()
