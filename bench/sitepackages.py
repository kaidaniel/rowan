"""How fast rowan loads installed packages (`--follow`), and what following them buys.

Nine scenarios, each a small realistic main program in bench/sitepackages/ and a venv made by
uv with every distribution pinned (installed with --no-deps, so the venv is the same on every
machine; cached under bench/work/sitepackages/, remade when the pins or the local package
change):

  s1_small_deps  dateutil, tabulate, humanize (attrs: modelled, never followed)
  s2_internal    acme_billing, a company-internal billing package (bench/sitepackages/
                 acme_billing_pkg: dataclasses, dicts, validation, pricing, report rows),
                 installed into site-packages as a wheel; dateutil and tabulate beside it
  s3_cli         click and rich
  s4_web         flask (+ werkzeug, jinja2, …) with pydantic (modelled, never followed)
  s5_data        numpy, pandas (modelled, never followed), scikit-learn (+ scipy, joblib)
  s6_symbolic    sympy (+ mpmath): a heavy import
  s7_orm         SQLAlchemy 2 on an in-memory SQLite database: an ORM's registries
  s8_schema      marshmallow: schemas, fields, hooks (httpx and requests are modelled)
  s9_graph       networkx: dispatching decorators on hundreds of functions

Modes (each one run of `rowan MAIN --facts --site-packages VENV`, ROWAN_DEBUG=1):

  none      no following (the default today)
  follow    --follow '*' as it is: per-function budgets, each entry's allowance (`fanout`
            budgets of work, then its calls do not run), the 10 s deadline
  only      --follow of the scenario's own package alone (s2: acme_billing), where one is named
  uncapped  --follow '*' with no entry allowance and no deadline, per-function budgets in force
            (ROWAN_LIMITS=fanout=10^9,ms=10^9), cut by --uncapped-timeout
  unbounded --follow '*' with every bound off (ROWAN_LIMITS=budget=10^9,ms=10^9: no
            per-function budgets, so no allowance either, counted in budgets), cut by
            --uncapped-timeout
            (uncapped and unbounded run only when asked: --uncapped)

Per run: wall time and peak RSS (/usr/bin/time), the `work:` counters, site-packages modules and
lines parsed, library entries (run / cut past their allowance), whether the deadline cut the
analysis.

What following buys, against `none` (the hover facts of the main file, `--facts`): each fact (a
name on a line, as a hover shows it) gets a precision level from its value:
  unknown  `?`, or only objects rowan does not see (an unknown, a library's object)
  partial  a known shape with unknown parts: `int(?)`, `list[?]`, `Customer(name=str(?))`,
           `{"C-1": customer_from_dict(c)}` (an unknown named by the call it comes from)
  exact    no `?`, no unknown object, no named unknown
`gained` / `lost` count facts whose level rose / fell, `changed` those whose text changed at
the same level. Names of library modules, classes and functions (`tabulate`: a library object
→ `function`; the program's function a library's decorator wraps, which without following is
taken unchanged: `cli` → `Group(…)`) are counted apart (`names`). Verdicts are counted by mark (✓ ✓~ ⚠ ⚠~ ✗ ◌);
`new_warn` counts accesses whose mark is ⚠ / ⚠~ / ✗ in a mode but not without following.

The check: following must never be worse than not following. The script exits with status 1
(after its table) when a capped mode (follow, only) loses a fact against `none` in any scenario
(`--no-check`: it does not).

Usage: python bench/sitepackages.py [--rowan PATH] [--uncapped] [--uncapped-timeout S]
           [--timeout S] [--scenarios s1,s2,…] [--modes none,follow,…] [-v] [--json] [--out FILE]
           [--no-check]
Every run goes through .guard/run when it exists (the owner's Mac); run from anywhere.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
PROGS = HERE / "sitepackages"
CACHE = HERE / "work" / "sitepackages"
GUARD = ROOT / ".guard" / "run"
PYTHON = "3.14"

# every distribution pinned (resolved once, then frozen): `uv pip install --no-deps`
SCENARIOS = {
    "s1_small_deps": {
        "pins": ["attrs==26.1.0", "humanize==4.16.0", "python-dateutil==2.9.0.post0", "six==1.17.0", "tabulate==0.10.0"],
    },
    "s2_internal": {
        "pins": ["python-dateutil==2.9.0.post0", "six==1.17.0", "tabulate==0.10.0"],
        "local": ["acme_billing_pkg"],
        "only": "acme_billing",
    },
    "s3_cli": {
        "pins": ["click==8.5.0", "markdown-it-py==4.2.0", "mdurl==0.1.2", "pygments==2.21.0", "rich==15.0.0"],
    },
    "s4_web": {
        "pins": [
            "annotated-types==0.8.0", "blinker==1.9.0", "click==8.5.0", "flask==3.1.3", "itsdangerous==2.2.0",
            "jinja2==3.1.6", "markupsafe==3.0.4", "pydantic==2.14.1", "pydantic-core==2.50.1",
            "typing-extensions==4.16.0", "typing-inspection==0.4.4", "werkzeug==3.1.9",
        ],
    },
    "s5_data": {
        "pins": [
            "cloudpickle==3.1.2", "joblib==1.6.0", "narwhals==2.27.1", "numpy==2.5.4", "pandas==3.0.6",
            "python-dateutil==2.9.0.post0", "scikit-learn==1.9.1", "scipy==1.18.1", "six==1.17.0", "threadpoolctl==3.7.0",
        ],
    },
    "s6_symbolic": {
        "pins": ["mpmath==1.3.0", "sympy==1.14.0"],
    },
    "s7_orm": {
        "pins": ["sqlalchemy==2.1.1", "typing-extensions==4.16.0"],
    },
    "s8_schema": {
        "pins": ["marshmallow==4.3.1"],
    },
    "s9_graph": {
        "pins": ["networkx==3.7"],
    },
}

LIMITS = {"uncapped": "fanout=1000000000,ms=1000000000", "unbounded": "budget=1000000000,ms=1000000000"}
WORK = re.compile(r"^work: (\d+) \(units (\d+), tabulations (\d+), objects (\d+), (\d+) ms; budgeted tabulations \d+, row joins (\d+), visits (\d+) .*havoc visits (\d+)\)")
LOADED = re.compile(r"^loaded: (\d+) modules, (\d+) lines \(site-packages: (\d+) modules, (\d+) lines\)")
ENTRIES = re.compile(r"^library entries \(run, cut\): \[(\d+), (\d+)\], work (\d+)")
# /usr/bin/time's wall clock (macOS `-l`, GNU `-v`): the guard's polling is not the run's time
REAL = re.compile(r"^\s*(?:([\d.]+) real|Elapsed \(wall clock\) time .*: ([\d:.]+)$)")
MARKS = {"proven": "✓", "likely": "✓~", "possibly": "⚠", "datakey": "⚠", "likely-missing": "⚠~", "missing": "✗", "input": "◌"}
UNSEEN = {"unknown", "external"}


def venv_for(name: str, spec: dict) -> Path:
    """The scenario's venv (made with uv, cached by a hash of its pins and local packages)."""
    h = hashlib.sha256(json.dumps([PYTHON, spec["pins"]]).encode())
    for local in spec.get("local", []):
        for f in sorted((PROGS / local).rglob("*")):
            if f.is_file() and "__pycache__" not in f.parts:
                h.update(str(f.relative_to(PROGS)).encode() + f.read_bytes())
    stamp = h.hexdigest()[:16]
    d = CACHE / name
    if (d / "stamp").exists() and (d / "stamp").read_text() == stamp:
        return d / "venv"
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    print(f"[{name}] making its venv", file=sys.stderr)
    subprocess.run(["uv", "venv", "-q", "--python", PYTHON, str(d / "venv")], check=True)
    pkgs = spec["pins"] + [str(PROGS / p) for p in spec.get("local", [])]
    if pkgs:
        py = d / "venv" / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        subprocess.run(["uv", "pip", "install", "-q", "--no-deps", "--python", str(py), *pkgs], check=True)
    (d / "stamp").write_text(stamp)
    return d / "venv"


def site_packages(venv: Path) -> Path:
    found = sorted(venv.glob("lib/python3*/site-packages")) or [venv / "Lib" / "site-packages"]
    return found[0]


def run_rowan(rowan: str, main: Path, site: Path, follow: str | None, limits: str | None, timeout: float) -> dict:
    """One analysis: its cost (wall, RSS, counters) and its --facts document."""
    cmd = [rowan, str(main), "--facts", "--site-packages", str(site)] + (["--follow", follow] if follow else [])
    timer = ["/usr/bin/time", "-l"] if sys.platform == "darwin" else ["/usr/bin/time", "-v"]
    if Path(timer[0]).exists():
        cmd = timer + cmd
    if GUARD.exists():
        cmd = [str(GUARD), "-t", str(int(timeout) + 1)] + cmd
    env = dict(os.environ, ROWAN_DEBUG="1")
    env.pop("ROWAN_LIMITS", None)
    if limits:
        env["ROWAN_LIMITS"] = limits
    t0 = time.monotonic()
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env, cwd=ROOT, start_new_session=True)
    try:
        out, err = p.communicate(timeout=timeout + 10)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, 9)
        out, err = p.communicate()
    wall = time.monotonic() - t0
    r = {"wall_s": round(wall, 2), "finished": p.returncode == 0}
    if not r["finished"]:
        r["why"] = next((l for l in err.splitlines() if l.startswith("GUARD")), f"exit {p.returncode}")
        if "over" in r["why"] or p.returncode in (124, -9):
            r["why"] = f"did not finish within {int(timeout)} s"
    for line in err.splitlines():
        if m := WORK.match(line):
            r.update(zip(["statements", "units", "tabulations", "objects", "engine_ms", "row_joins", "visits", "havoc"], map(int, m.groups())))
        elif m := LOADED.match(line):
            r.update(zip(["modules", "lines", "site_modules", "site_lines"], map(int, m.groups())))
        elif m := ENTRIES.match(line):
            r.update(zip(["entries_run", "entries_cut", "lib_work"], map(int, m.groups())))
        elif m := REAL.match(line):
            t = m.group(1) or m.group(2)
            r["wall_s"] = round(sum(float(x) * 60**i for i, x in enumerate(reversed(t.split(":")))), 2)
        elif "maximum resident set size" in line.lower():
            n = int(line.split()[0]) if sys.platform == "darwin" else int(line.split()[-1]) * 1024
            r["rss_mb"] = round(n / 2**20)
    if r["finished"]:
        try:
            doc = json.loads(out)
            r["facts"], r["verdicts"] = doc["facts"], doc["verdicts"]
            r["cut"] = bool(doc["analysis"]["cut"])
        except (json.JSONDecodeError, KeyError):
            r["finished"], r["why"] = False, "no facts document"
    return r


def level(fact: dict) -> int:
    """0 unknown, 1 partial, 2 exact (see the module's doc)."""
    text, kinds = fact["text"], [o["kind"] for o in fact["objects"]]
    if text == "?" or (kinds and all(k in UNSEEN for k in kinds)):
        return 0
    # (a legend names the unknowns the text shows by where they come from: `customer_from_dict(c)`;
    # `named` those it names with nothing more known: `nx.connected_components(g)[*]`, `ctx`)
    return 1 if "?" in text or any(k in UNSEEN for k in kinds) or fact.get("legend") or fact.get("named") else 2


def is_name(fact: dict) -> bool:
    """A library's module, class or function as such (a hover of an imported name)."""
    kinds = {o["kind"] for o in fact["objects"]}
    return bool(kinds) and kinds <= {"external", "module", "class", "function"}


def compare(base: dict, run: dict, show: str | None = None) -> dict:
    """What a mode's facts and verdicts gained or lost against no following (`show`: print
    each gained / lost fact and new warning to stderr under this heading)."""
    key = lambda f: (f["line"], f["name"])
    a, b = {key(f): f for f in base["facts"]}, {key(f): f for f in run["facts"]}
    c = Counter()
    for k in sorted(a.keys() | b.keys()):
        fa, fb = a.get(k), b.get(k)
        if fa is None or fb is None:
            c["new" if fa is None else "gone"] += 1
        elif fa["text"] == fb["text"] and level(fa) == level(fb):
            continue
        elif is_name(fa):
            # (a library's class or function as such, or the program's function a library's
            # decorator wraps, taken unchanged without following: what following shows it is)
            c["names"] += 1
        else:
            la, lb = level(fa), level(fb)
            what = "gained" if lb > la else "lost" if lb < la else "changed"
            c[what] += 1
            if show and what != "changed":
                print(f"{show} {what} L{k[0]} {k[1]}: {fa['text'][:70]}  ->  {fb['text'][:70]}", file=sys.stderr)
    warn = lambda vs: {(v["file"], v["line"], v["col"], v["key"]): v for v in vs if MARKS.get(v["tier"], "") in ("⚠", "⚠~", "✗")}
    new = warn(run["verdicts"]).keys() - warn(base["verdicts"]).keys()
    c["new_warn"] = len(new)
    for k in sorted(new) if show else []:
        v = warn(run["verdicts"])[k]
        print(f"{show} new {MARKS[v['tier']]} L{v['line']} {v['text']}: {v['message']}", file=sys.stderr)
    return dict(c)


def summary(r: dict) -> dict:
    out = {k: v for k, v in r.items() if k not in ("facts", "verdicts")}
    if r.get("finished"):
        out["facts"] = len(r["facts"])
        out["levels"] = dict(Counter(["unknown", "partial", "exact"][level(f)] for f in r["facts"] if not is_name(f)))
        out["marks"] = dict(Counter(MARKS.get(v["tier"], v["tier"]) for v in r["verdicts"] if v["file"] == ""))
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--rowan", default=str(ROOT / "rowan" / "target" / "release" / "rowan"))
    ap.add_argument("--scenarios", default=",".join(SCENARIOS))
    ap.add_argument("--modes", default=None, help="none,follow,only,uncapped,unbounded (default: all but the last two)")
    ap.add_argument("--uncapped", action="store_true", help="also run the uncapped and unbounded modes")
    ap.add_argument("--timeout", type=float, default=60, help="per run of the capped modes (s)")
    ap.add_argument("--uncapped-timeout", type=float, default=120)
    ap.add_argument("-v", "--verbose", action="store_true", help="list each fact gained or lost, each new warning")
    ap.add_argument("--json", action="store_true", help="print the results as JSON, not a table")
    ap.add_argument("--out", help="also write the JSON here")
    ap.add_argument("--no-check", action="store_true", help="do not fail when following loses facts")
    a = ap.parse_args()
    modes = a.modes.split(",") if a.modes else ["none", "follow", "only"] + (["uncapped", "unbounded"] if a.uncapped else [])
    results = {}
    for name in a.scenarios.split(","):
        name = next(s for s in SCENARIOS if s == name or s.startswith(name + "_"))
        spec = SCENARIOS[name]
        site = site_packages(venv_for(name, spec))
        main_py = PROGS / f"{name}.py"
        runs = {}
        for mode in ["none"] + [m for m in modes if m != "none"]:
            if mode == "only" and "only" not in spec:
                continue
            follow = {"none": None, "follow": "*", "only": spec.get("only"), "uncapped": "*", "unbounded": "*"}[mode]
            limits = LIMITS.get(mode)
            timeout = a.uncapped_timeout if mode in LIMITS else a.timeout
            print(f"[{name}] {mode}", file=sys.stderr)
            runs[mode] = run_rowan(a.rowan, main_py, site, follow, limits, timeout)
        base = runs["none"]
        results[name] = {}
        for mode, r in runs.items():
            if mode not in modes:
                continue
            s = summary(r)
            if r.get("finished") and base.get("finished") and mode != "none":
                s["vs_none"] = compare(base, r, f"[{name} {mode}]" if a.verbose else None)
            results[name][mode] = s
    doc = {"rowan": a.rowan, "python": PYTHON, "scenarios": results}
    if a.out:
        Path(a.out).write_text(json.dumps(doc, indent=2, ensure_ascii=False))
    if a.json:
        print(json.dumps(doc, indent=2, ensure_ascii=False))
    else:
        print(table(results))
    # (following must never be worse than not following)
    lost = [(name, mode, s["vs_none"]["lost"]) for name, ms in results.items() for mode, s in ms.items() if mode in ("follow", "only") and s.get("vs_none", {}).get("lost")]
    if lost and not a.no_check:
        print("following loses facts against not following: " + ", ".join(f"{n} {m}: {k}" for n, m, k in lost) + " (-v lists them)", file=sys.stderr)
        sys.exit(1)


def table(results: dict) -> str:
    """A Markdown table: one row per scenario and mode."""
    head = "| scenario | mode | time | peak RSS | statements | units | site-packages parsed | entries run / cut | facts unknown / partial / exact | vs none: gained / lost / changed | new ⚠✗ | marks |"
    cols = head.count("|") - 1
    rows = [head, "|" + " --- |" * cols]
    for name, modes in results.items():
        for mode, s in modes.items():
            if not s.get("finished"):
                rows.append(f"| {name} | {mode} | {s.get('why', 'failed')} |" + " |" * (cols - 3))
                continue
            lv = s.get("levels", {})
            vs = s.get("vs_none")
            cmp = f"{vs.get('gained', 0)} / {vs.get('lost', 0)} / {vs.get('changed', 0)}" if vs else "–"
            marks = " ".join(f"{k}{v}" for k, v in sorted(s.get("marks", {}).items()))
            cut = " (cut)" if s.get("cut") else ""
            rows.append(
                f"| {name} | {mode} | {s['wall_s']:.2f} s{cut} | {s.get('rss_mb', '?')} MB | {s.get('statements', '?')} | {s.get('units', '?')} "
                f"| {s.get('site_modules', 0)} modules, {s.get('site_lines', 0)} lines "
                f"| {s.get('entries_run', 0)} / {s.get('entries_cut', 0)} "
                f"| {lv.get('unknown', 0)} / {lv.get('partial', 0)} / {lv.get('exact', 0)} | {cmp} | {vs.get('new_warn', 0) if vs else '–'} | {marks} |"
            )
    return "\n".join(rows)


if __name__ == "__main__":
    main()
