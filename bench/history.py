"""Turn bench/run.py results into github-action-benchmark's `customSmallerIsBetter` format.

Usage: python bench/history.py RESULTS_DIR OUT.json

Also the on-demand hover timings (hovers.py's NAME-hovers.json: setup, p50 / p99 / max per
function; first_hover.py's NAME-first.json: p50 / max time to the first hover, peak memory) and
the site-packages benchmark (sitepackages.py's sitepackages.json: per scenario and mode, wall
time, work, peak memory and the main file's hover facts still unknown).

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
        if f.name.endswith("-hovers.json"):
            h = r["summary"]
            note = f"{h['timed']} functions on demand"
            for key, label in [("setup_ms", "setup"), ("p50", "p50"), ("p99", "p99"), ("max", "max")]:
                if h.get(key) is not None:
                    entries.append({"name": f"{h['name']} hovers: {label}", "unit": "ms", "value": h[key], "extra": note})
            continue
        if f.name.endswith("-first.json"):
            h = r["summary"]
            note = f"{h['modules']} modules, {h['on_demand']} on demand"
            for key in ("p50", "max"):
                entries.append({"name": f"{h['name']} first hover: {key}", "unit": "ms", "value": h["first_hover_ms"][key], "extra": note})
            entries.append({"name": f"{h['name']} first hover: peak memory", "unit": "MB", "value": h["peak_mb"], "extra": note})
            continue
        if "scenarios" in r:
            for scen, modes in r["scenarios"].items():
                for mode, m in modes.items():
                    if mode == "none":
                        continue
                    note = "finished" if m.get("finished") else "cut"
                    for key, label, unit in [("wall_s", "wall time", "s"), ("statements", "work", "statements"), ("rss_mb", "peak memory", "MB")]:
                        if m.get(key) is not None:
                            entries.append({"name": f"site-packages {scen} {mode}: {label}", "unit": unit, "value": m[key], "extra": note})
                    if m.get("levels"):
                        entries.append({"name": f"site-packages {scen} {mode}: unknown hovers", "unit": "facts", "value": m["levels"].get("unknown", 0), "extra": note})
            continue
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
