"""Run rowan once on a benchmark program without its deadlines and record what it cost
(neither the clock-based deadlines nor the work-based ones: `work=0` and huge `ms` / `hard`).

Usage: python bench/run.py NAME ROWAN FILE [--timeout S] [--perf OUT.data] [--out DIR]

Records wall time, peak memory, rowan's `work:` line (deterministic counters: the best
regression signal on noisy CI machines), the number of ✗ / ⚠ verdicts, and whether the run
finished before the timeout. With --perf the run is recorded by `perf record -g` (Linux).
Writes DIR/NAME.json and DIR/NAME.md (a table row for the job summary).
"""

import argparse
import json
import os
import re
import resource
import signal
import subprocess
import sys
import time
from pathlib import Path

WORK = re.compile(r"^work: (\d+) \((?:units (\d+), )?tabulations (\d+), objects (\d+), (\d+) ms")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("name")
    ap.add_argument("rowan")
    ap.add_argument("file")
    ap.add_argument("--timeout", type=float, default=300)
    ap.add_argument("--perf")
    ap.add_argument("--out", default="bench-results")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)

    cmd = [a.rowan, a.file, "--no-rows"]
    if a.perf:
        cmd = ["perf", "record", "-F", "199", "-g", "-o", a.perf, "--"] + cmd
    env = dict(os.environ, ROWAN_DEBUG="1", ROWAN_LIMITS="ms=100000000")
    stdout_path, stderr_path = out / f"{a.name}.out", out / f"{a.name}.err"
    t0 = time.monotonic()
    with open(stdout_path, "w") as so, open(stderr_path, "w") as se:
        p = subprocess.Popen(cmd, stdout=so, stderr=se, env=env, start_new_session=True)
        try:
            p.wait(timeout=a.timeout)
            finished = p.returncode == 0
        except subprocess.TimeoutExpired:
            finished = False
            # SIGINT first: perf stops the workload and still writes its data
            os.killpg(p.pid, signal.SIGINT)
            try:
                p.wait(timeout=60)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)
                p.wait()
    wall = time.monotonic() - t0
    maxrss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    rss_mb = maxrss / 1024 if sys.platform.startswith("linux") else maxrss / (1024 * 1024)

    work = None
    for line in stderr_path.read_text(errors="replace").splitlines():
        m = WORK.match(line)
        if m:
            work = {k: int(v) for k, v in zip(["work", "units", "tabulations", "objects", "ms"], m.groups()) if v is not None}
    text = stdout_path.read_text(errors="replace")
    verdicts = sum(1 for l in text.splitlines() if "✗" in l or "⚠" in l)

    res = {
        "name": a.name,
        "finished": finished,
        "returncode": p.returncode,
        "timeout_s": a.timeout,
        "wall_s": round(wall, 2),
        "peak_rss_mb": round(rss_mb),
        "verdicts": verdicts if finished else None,
        **(work or {}),
    }
    (out / f"{a.name}.json").write_text(json.dumps(res, indent=2))
    status = "finished" if finished else f"**timeout** ({a.timeout:.0f} s)" if p.returncode in (-2, -9, 130) or not work else f"exit {p.returncode}"
    w = lambda k: f"{res[k]:,}" if k in res else "–"
    row = f"| {a.name} | {status} | {res['wall_s']:.1f} s | {res['peak_rss_mb']:,} MB | {w('work')} | {w('units')} | {w('tabulations')} | {w('objects')} | {verdicts if finished else '–'} |"
    (out / f"{a.name}.md").write_text(row + "\n")
    print(json.dumps(res, indent=2))


if __name__ == "__main__":
    main()
