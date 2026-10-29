"""Turn bench.yml's results into github-action-benchmark's `customSmallerIsBetter` format.

Usage: python bench/history.py RESULTS_DIR OUT.json

What it reads in RESULTS_DIR (each job's artifact, merged):
  tracked-*.json   bench/tracked.py metrics (the 31 programs, CI's shards): per program and mode
                   wall time, peak memory, work, work units, tabulations, objects. The no-deadline
                   mode's series are "NAME: METRIC" (the names bench/run.py's runs of the eight had
                   until v0.1.103, then without --facts), the 10 s deadline's "NAME, 10 s deadline:
                   METRIC"; a cut run's wall time is its timeout, marked in the entry's `extra`
  *-hovers.json    bench/hovers.py: setup, p50 / p99 / max per function
  *-first.json     bench/first_hover.py: p50 / max time to the first hover, peak memory
  sitepackages.json bench/sitepackages.py: per scenario and mode, wall time, work, peak memory and
                   the main file's hover facts still unknown
  golden-tab0.json the golden suites at budget=0: failing cases
  protocols-*.json bench/protocols.py full (CI's jobs): per enumerator the points missed and the
                   seconds, summed over its shards; and the totals
  metamorphic-*.json tests/metamorphic.py --json (CI's shards): places missed per check, summed
  kani.json        `cargo kani --lib`: harnesses failed, seconds

One entry per series; metrics a run did not produce are left out.
"""

import json
import sys
from collections import Counter
from pathlib import Path

METRICS = [
    ("wall_s", "wall time", "s"),
    ("rss_mb", "peak memory", "MB"),
    ("work", "work", "statements"),
    ("units", "work units", "units"),
    ("tabulations", "tabulations", "count"),
    ("objects", "objects", "count"),
]


def tracked_entries(r: dict) -> list[dict]:
    out = []
    for mode, res in r.get("modes", {}).items():
        suffix = "" if mode == "nodeadline" else ", 10 s deadline"
        for name, run in res["runs"].items():
            note = ("finished" if run.get("finished") else f"cut at the {res.get('timeout_s', 0):.0f} s timeout") + (", analysis cut" if run.get("cut") else "")
            for key, label, unit in METRICS:
                v = run.get(key)
                # (memory and counters of a run the timeout stopped say nothing)
                if v is None or (key != "wall_s" and not run.get("finished")):
                    continue
                out.append({"name": f"{name}{suffix}: {label}", "unit": unit, "value": v, "extra": note})
    return out


def main() -> None:
    results, out = Path(sys.argv[1]), Path(sys.argv[2])
    entries = []
    prot, meta = {}, Counter()
    meta_seen = False
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
        if r.get("set") == "tracked":
            entries += tracked_entries(r)
            continue
        if r.get("kind") == "full" and "by_name" in r:
            for n, t in r["by_name"].items():
                p = prot.setdefault(n, Counter())
                p.update({"missed": t["missed"] + t["crashed"], "seconds": t["seconds"], "points": t["points"]})
            continue
        if f.name.startswith("metamorphic") and "missed" in r:
            meta_seen = True
            meta.update({k: v for k, v in r["missed"].items()})
            meta["seconds"] += r.get("seconds", 0)
            continue
        if f.name == "kani.json":
            entries.append({"name": "kani: failed harnesses", "unit": "harnesses", "value": r.get("failed", 0), "extra": f"{r.get('harnesses')} harnesses"})
            entries.append({"name": "kani: seconds", "unit": "s", "value": r.get("seconds", 0), "extra": ""})
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
            # the golden suites with every call past its budget (`budget=0`): failing cases
            entries.append({"name": "golden tab=0: failing cases", "unit": "cases", "value": r["golden_failing"], "extra": f"{r['golden_passing']} passing"})
            continue
    for n, p in sorted(prot.items()):
        note = f"{p['points']:,} points"
        entries.append({"name": f"protocol {n} --all: missed points", "unit": "points", "value": p["missed"], "extra": note})
        entries.append({"name": f"protocol {n} --all: seconds", "unit": "s", "value": round(p["seconds"], 1), "extra": note})
    if prot:
        entries.append({"name": "protocols --all: missed points", "unit": "points", "value": sum(p["missed"] for p in prot.values()), "extra": f"{len(prot)} enumerators"})
    if meta_seen:
        secs = meta.pop("seconds")
        entries.append({"name": "metamorphic: missed places", "unit": "places", "value": sum(meta.values()), "extra": ", ".join(f"{k} {v}" for k, v in sorted(meta.items()))})
        entries.append({"name": "metamorphic: seconds", "unit": "s", "value": round(secs, 1), "extra": "all shards"})
    out.write_text(json.dumps(entries, indent=2))
    print(f"{len(entries)} entries")


if __name__ == "__main__":
    main()
