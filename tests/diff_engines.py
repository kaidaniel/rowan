"""Differential check of the two engines (SPECIFICATION.md §15.4): the Python prototype is the
oracle; the Rust engine in compat mode must produce the same dedupe'd verdict set
(file, line, key, tier, namespace) and the same schema table on every program.

    python tests/diff_engines.py                 # golden cases, the example, the prototype's own modules
    python tests/diff_engines.py prog.py ...     # specific files

Set ROWAN_RS to the engine binary (default: rowan-rs/target/release/rowan-rs, then debug).
"""
from __future__ import annotations

import glob
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)

from rowan import analyse_file  # noqa: E402
from rowan.domain import key_str  # noqa: E402
from rowan.report import dedupe, schema_rows  # noqa: E402

RUST_BIN = os.environ.get("ROWAN_RS", os.path.join(ROOT, "rowan-rs", "target", "release", "rowan-rs"))
if not os.path.exists(RUST_BIN):
    RUST_BIN = os.path.join(ROOT, "rowan-rs", "target", "debug", "rowan-rs")

DEFAULT = (sorted(glob.glob(os.path.join(HERE, "cases", "*.py")) + glob.glob(os.path.join(HERE, "cases", "*", "*.py")))
           + [os.path.join(ROOT, "examples", "etl_report.py")]
           + [os.path.join(ROOT, "rowan", m + ".py") for m in ("scopes", "layered", "domain", "report", "state", "summary")])


def main(argv):
    paths = argv or DEFAULT
    paths = [p for p in paths if not os.path.basename(p).startswith("helpers")]
    same = diff = 0
    for p in paths:
        with open(p, encoding="utf-8") as f:
            head = f.read(200)
        if "#@engine rust" in head:
            continue
        an = analyse_file(p)
        py = {(v.file, v.line, key_str(v.key), v.tier, v.ns) for v in dedupe(an.verdicts)}
        pys = {(src, k, e["always"]) for src, d in schema_rows(an).items() for k, e in d.items()}
        out = subprocess.run([RUST_BIN, p, "--golden", "--compat"], capture_output=True, text=True)
        if out.returncode != 0:
            print(f"FAIL  {os.path.relpath(p, ROOT)}: rowan-rs exited {out.returncode}: {out.stderr.strip()[-300:]}")
            diff += 1
            continue
        res = json.loads(out.stdout)
        rs = {(v["file"], v["line"], v["key"], v["tier"], v["ns"]) for v in res["verdicts"]}
        rss = {(src, k, e["always"]) for src, d in res["schemas"].items() for k, e in d.items()}
        name = os.path.relpath(p, ROOT)
        deltas = [("py only", sorted(py - rs)), ("rs only", sorted(rs - py)),
                  ("py schema only", sorted(pys - rss)), ("rs schema only", sorted(rss - pys))]
        if not any(d for _, d in deltas):
            same += 1
            print(f"SAME  {name}  ({len(py)} verdicts, {len(pys)} schema entries)")
            continue
        diff += 1
        print(f"DIFF  {name}")
        for label, d in deltas:
            for x in d:
                print(f"    {label}: {x}")
    print(f"{same} same, {diff} differ")
    return 0 if diff == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
