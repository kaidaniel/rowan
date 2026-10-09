"""Golden-case harness for the rowan engine (rowan/target/release/rowan, then debug).

Expectations are comments inside the case files:

    r["role"]                 #@ possibly role
    r["rolle"]                #@ missing rolle ; none tenant
    d = build()               #@row d: present={id,role} definite={id} closed
    #@schema read_csv@L1: always={qty,price} sometimes={email}

Tiers: ok|proven, likely, possibly, missing, likely-missing, input, datakey, none (no verdict for
that key at this line), any (some verdict for the key), sound (no `missing` verdict for the
key: the key may really be there, so only that would be wrong; `likely-missing`, "missing
unless unknown code added it", is a hedged and true statement).
Row checks beyond present/definite/closed/open: `possible={..}` (each key is present, or the
row is open) and `definite_within={..}` (every definite key is listed): soundness floors for
cases whose precise answer is a `#@todo`.  Keys are bare words or Python literals.
A verdict matches a line if it was recorded at that line or while a call made from that line was
active.  A `#@` directive must be the last thing on its line (the key runs to its end).

    python tests/run_golden.py [-v] [case…]
"""
from __future__ import annotations

import ast
import glob
import json
import os
import re
import subprocess
import sys
from typing import Any, Dict, List, Tuple

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

TODO_COUNT = [0]  # `#@todo` directives seen: expectations the engine does not meet yet

TIER_ALIASES = {"ok": "proven", "proven": "proven", "likely": "likely", "possibly": "possibly",
                "missing": "missing", "likely-missing": "likely-missing", "input": "input",
                "datakey": "datakey", "none": "none", "any": "any", "map": "none", "sound": "sound"}


def key_str(k: Any) -> str:
    return repr(k)


def parse_key(tok: str) -> Any:
    tok = tok.strip()
    try:
        return ast.literal_eval(tok)
    except Exception:
        return tok


def parse_set(s: str) -> set:
    s = s.strip()
    if s.startswith("{") and s.endswith("}"):
        s = s[1:-1]
    return {parse_key(t) for t in s.split(",") if t.strip()}


def collect_expectations(src: str):
    verdict_exp: Dict[int, List[Tuple[str, Any]]] = {}
    row_exp: Dict[int, List[Tuple[str, Dict[str, Any]]]] = {}
    schema_exp: List[Tuple[str, Dict[str, Any]]] = []
    for i, line in enumerate(src.splitlines(), 1):
        if "#@todo" in line:
            TODO_COUNT[0] += 1
            if os.environ.get("ROWAN_TODO") != "check":
                continue
            # ROWAN_TODO=check: the open expectations are checked like the others
            line = re.sub(r"#@todo\s+row\b", "#@row", line)
            line = re.sub(r"#@todo\s+schema\b", "#@schema", line)
            line = re.sub(r"#@todo\s+", "#@ ", line)
        m = re.search(r"#@row\s+(\w+)\s*:\s*(.*)$", line)
        if m:
            spec: Dict[str, Any] = {}
            for part in re.findall(r"(\w+)=(\{[^}]*\})|(\bclosed\b|\bopen\b)", m.group(2)):
                if part[0]:
                    spec[part[0]] = parse_set(part[1])
                elif part[2]:
                    spec[part[2]] = True
            row_exp.setdefault(i, []).append((m.group(1), spec))
            continue
        m = re.search(r"#@schema\s+([^:]+):\s*(.*)$", line)
        if m:
            spec = {}
            for part in re.findall(r"(\w+)=(\{[^}]*\})", m.group(2)):
                spec[part[0]] = parse_set(part[1])
            if "none" in m.group(2).split():
                spec["none"] = True
            schema_exp.append((m.group(1).strip(), spec))
            continue
        m = re.search(r"#@\s+(.*)$", line)
        if m:
            for part in m.group(1).split(";"):
                toks = part.strip().split(None, 1)
                if not toks:
                    continue
                tier = TIER_ALIASES.get(toks[0].strip())
                if tier is None:
                    raise ValueError(f"line {i}: unknown tier {toks[0]!r}")
                key = parse_key(toks[1]) if len(toks) > 1 else None
                verdict_exp.setdefault(i, []).append((tier, key))
    return verdict_exp, row_exp, schema_exp


RUST_BIN = os.environ.get("ROWAN_BIN", os.path.join(ROOT, "rowan", "target", "release", "rowan"))
if not os.path.exists(RUST_BIN):
    RUST_BIN = os.path.join(ROOT, "rowan", "target", "debug", "rowan")


def check_results(verdict_exp, row_exp_checks, schema_exp, vs, sch) -> List[str]:
    """Shared expectation checker.  `vs`: list of dicts with line/chain/key/key_str/tier;
    `row_exp_checks`: [(line, name, spec, problem)]; `sch`: {label: {key_repr: {"always": bool}}}."""
    failures: List[str] = []
    for ln, exps in sorted(verdict_exp.items()):
        here = [v for v in vs if v["line"] == ln or ln in v["chain"]]
        for tier, key in exps:
            if tier == "none" and key is None:
                bad = [v for v in here if v["tier"] not in ("proven",)]
                if bad:
                    failures.append(f"L{ln}: expected no diagnostics, got " + ", ".join(f"{v['tier']} {v['key']}" for v in bad))
                continue
            matches = [v for v in here if v["key"] == key_str(key) or (isinstance(key, str) and v["key_str"].startswith(key))]
            if tier == "none":
                if matches:
                    failures.append(f"L{ln}: expected no verdict for {key!r}, got " + ", ".join(v["tier"] for v in matches))
                continue
            if tier == "sound":
                wrong = [v for v in matches if v["tier"] == "missing"]
                if wrong:
                    failures.append(f"L{ln}: {key!r} may be present, but rowan says " + ", ".join(sorted({v['tier'] for v in wrong})))
                continue
            if tier == "any":
                if not matches:
                    failures.append(f"L{ln}: expected some verdict for {key!r}, got none")
                continue
            if not matches:
                got = ", ".join(f"{v['tier']} {v['key']}" for v in here) or "nothing"
                failures.append(f"L{ln}: expected {tier} {key!r}; got {got}")
            elif not any(v["tier"] == tier for v in matches):
                failures.append(f"L{ln}: expected {tier} {key!r}; got " + ", ".join(sorted({v["tier"] for v in matches})))
    for ln, name, spec, problem in row_exp_checks:
        if problem:
            failures.append(f"L{ln} row {name}: {problem}")
    for label, spec in schema_exp:
        hits = {k: v for k, v in sch.items() if label in k}
        if spec.get("none"):
            keys = {k for e in hits.values() for k in e}
            if keys:
                failures.append(f"schema {label}: expected none, got {sorted(keys)}")
            continue
        if not hits:
            failures.append(f"schema {label}: no requirements recorded (have: {sorted(sch)})")
            continue
        merged: Dict[str, Any] = {}
        for e in hits.values():
            merged.update(e)
        always = {k for k, e in merged.items() if e["always"]}
        allk = set(merged)
        for k in spec.get("always", set()):
            if key_str(k) not in always:
                failures.append(f"schema {label}: {k!r} should be always; got {'sometimes' if key_str(k) in allk else 'nothing'}")
        for k in spec.get("sometimes", set()):
            if key_str(k) not in allk:
                failures.append(f"schema {label}: {k!r} missing")
            elif key_str(k) in always:
                failures.append(f"schema {label}: {k!r} should be sometimes, got always")
        if "only" in spec:
            extra = allk - {key_str(k) for k in spec["only"]}
            if extra:
                failures.append(f"schema {label}: unexpected keys {sorted(extra)}")
    return failures


def row_problems(row: Dict[str, Any], spec: Dict[str, Any]) -> str:
    """`row`: {"present": [repr...], "definite": [...], "absent": [...], "closed": bool}."""
    present = set(row["present"])
    definite = set(row["definite"])
    absent = set(row["absent"])
    problems = []
    if "present" in spec and present != {key_str(k) for k in spec["present"]}:
        problems.append(f"present={sorted(present)} want {sorted(key_str(k) for k in spec['present'])}")
    if "definite" in spec and definite != {key_str(k) for k in spec["definite"]}:
        problems.append(f"definite={sorted(definite)} want {sorted(key_str(k) for k in spec['definite'])}")
    if "absent" in spec and not {key_str(k) for k in spec["absent"]} <= absent:
        problems.append(f"absent={sorted(absent)} want ⊇ {sorted(key_str(k) for k in spec['absent'])}")
    if "possible" in spec and row["closed"]:
        lost = {key_str(k) for k in spec["possible"]} - present
        if lost:
            problems.append(f"possible keys {sorted(lost)} not in present={sorted(present)} of a closed row")
    if "definite_within" in spec:
        extra = definite - {key_str(k) for k in spec["definite_within"]}
        if extra:
            problems.append(f"definite={sorted(definite)} claims {sorted(extra)}, not definite in the program")
    if spec.get("closed") and not row["closed"]:
        problems.append(f"expected closed, got {row.get('details', row.get('text', row))}")
    if spec.get("open") and row["closed"]:
        problems.append(f"expected open, got {row.get('details', row.get('text', row))}")
    return "; ".join(problems)


def run_case(path: str, verbose: bool = False) -> Tuple[bool, List[str]]:
    with open(path, encoding="utf-8") as f:
        src = f.read()
    verdict_exp, row_exp, schema_exp = collect_expectations(src)
    # `#@argv ...`: extra engine arguments for this case (relative paths against its dir)
    argv: List[str] = []
    m = re.search(r"^#@argv\s+(.*)$", src, re.M)
    if m:
        for a in m.group(1).split():
            argv.append(os.path.join(os.path.dirname(path), a) if (a.startswith(".") or a.startswith("site")) and not a.startswith("--") else a)
    try:
        out = subprocess.run([RUST_BIN, path, "--golden", *argv], capture_output=True, text=True, timeout=600)
    except Exception as e:  # noqa: BLE001
        return False, [f"could not run {RUST_BIN} (build it with `cargo build --release` in rowan/): {e}"]
    if out.returncode != 0:
        return False, [f"rowan failed (exit {out.returncode}): {out.stderr.strip()[-2000:]}"]
    try:
        res = json.loads(out.stdout)
    except json.JSONDecodeError as e:
        return False, [f"bad JSON from rowan: {e}: {out.stdout[:200]}"]
    # a line can run several times (a function called from several places, or analysed as an
    # entry point with open parameters): its row is the join over the runs
    joined: Dict[Tuple[int, str], Any] = {}
    for rc in res["rows"]:
        key = (rc["line"], rc["name"])
        r = rc["row"]
        if key not in joined:
            joined[key] = None if r is None else dict(r, present=list(r["present"]), definite=list(r["definite"]), absent=list(r["absent"]))
        elif r is not None:
            j = joined[key]
            if j is None:
                joined[key] = dict(r)
                continue
            j["present"] = sorted(set(j["present"]) | set(r["present"]))
            j["definite"] = sorted(set(j["definite"]) & set(r["definite"]))
            j["absent"] = sorted(set(j["absent"]) & set(r["absent"]))
            j["closed"] = j["closed"] and r["closed"]
            if r["text"] != j["text"]:
                j["text"] = j["text"] + " | " + r["text"]
                j["details"] = j.get("details", "") + " | " + r.get("details", "")
    row_checks = []
    for (line, rname), row in joined.items():
        for name, spec in row_exp.get(line, []):
            if name != rname:
                continue
            if row is None:
                row_checks.append((line, name, spec, "no record object"))
            else:
                row_checks.append((line, name, spec, row_problems(row, spec)))
    seen_rows = set(joined)
    for ln, specs in row_exp.items():
        for name, spec in specs:
            if (ln, name) not in seen_rows:
                row_checks.append((ln, name, spec, "statement never executed"))
    failures = check_results(verdict_exp, row_checks, schema_exp, res["verdicts"], res["schemas"])
    if verbose or failures:
        print(f"\n===== {os.path.basename(path)} =====")
        print(res["report"])
    return not failures, failures


def main(argv: List[str]) -> int:
    verbose = "-v" in argv
    args = [a for a in argv if not a.startswith("-")]
    paths = args or sorted(glob.glob(os.path.join(HERE, "cases", "*.py"))
                           + glob.glob(os.path.join(HERE, "cases", "*", "*.py")))
    # in a package directory only the entry module is a case (its name shares the directory's
    # prefix: `k94_pkg/k94_modules.py`); the other files are modules it imports
    def is_case(p):
        d = os.path.basename(os.path.dirname(p))
        if d == "cases":
            return True
        prefix = d.split("_")[0]
        return os.path.basename(p).startswith(prefix + "_")
    paths = [p for p in paths if args or is_case(p)]  # explicit paths are always cases
    ok_all = True
    results = []
    for p in paths:
        ok, fails = run_case(p, verbose)
        results.append((os.path.basename(p), ok, fails))
        ok_all &= ok
    print("\n===== summary =====")
    npass = sum(1 for _n, ok, _f in results if ok)
    for name, ok, fails in results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}")
        for f in fails:
            print(f"        - {f}")
    print(f"  {npass}/{len(results)} passed" + (f"  ({TODO_COUNT[0]} #@todo expectations not checked)" if TODO_COUNT[0] else ""))
    return 0 if ok_all else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
