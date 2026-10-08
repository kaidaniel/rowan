"""Snapshot tests: the engine's `--json` output on larger inputs must not change unnoticed.

    python tests/run_snapshots.py            # compare against tests/snapshots/*.jsonl
    python tests/run_snapshots.py --update   # accept the current output

Inputs: examples/etl_report.py and tests/fixtures/*.py (large realistic programs).  A diff
is printed per changed verdict / schema entry; review it and run --update when it is
intended.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BIN = os.environ.get("ROWAN_BIN", os.path.join(ROOT, "rowan", "target", "release", "rowan"))
if not os.path.exists(BIN):
    BIN = os.path.join(ROOT, "rowan", "target", "debug", "rowan")
SNAP = os.path.join(HERE, "snapshots")
INPUTS = [os.path.join(ROOT, "examples", "etl_report.py")] + sorted(
    os.path.join(HERE, "fixtures", f) for f in os.listdir(os.path.join(HERE, "fixtures")) if f.endswith(".py"))


def analyse(path: str) -> list:
    out = subprocess.run([BIN, path, "--json", "--no-summaries"], capture_output=True, text=True, timeout=1800)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip()[-500:])
    recs = []
    for line in out.stdout.splitlines():
        r = json.loads(line)
        if r["kind"] == "verdict":
            recs.append(["verdict", r["file"], r["line"], r["col"], r["key"], r["ns"], r["tier"], r["chain"], r["object"]])
        elif r["kind"] == "schema":
            for k, e in sorted(r["keys"].items()):
                recs.append(["schema", r["source"], k, e["always"]])
    return sorted(recs, key=repr)


def main(argv):
    update = "--update" in argv
    os.makedirs(SNAP, exist_ok=True)
    ok = True
    for p in INPUTS:
        name = os.path.splitext(os.path.basename(p))[0] + ".jsonl"
        snap = os.path.join(SNAP, name)
        got = analyse(p)
        if update or not os.path.exists(snap):
            with open(snap, "w", encoding="utf-8") as f:
                for r in got:
                    f.write(json.dumps(r) + "\n")
            print(f"  WROTE {name} ({len(got)} records)")
            continue
        with open(snap, encoding="utf-8") as f:
            want = [json.loads(l) for l in f if l.strip()]
        if got == want:
            print(f"  SAME  {name} ({len(got)} records)")
            continue
        ok = False
        print(f"  DIFF  {name}")
        ws, gs = set(map(json.dumps, want)), set(map(json.dumps, got))
        for r in sorted(ws - gs)[:40]:
            print(f"    - {r}")
        for r in sorted(gs - ws)[:40]:
            print(f"    + {r}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
