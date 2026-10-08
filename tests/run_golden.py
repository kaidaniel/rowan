"""Golden-case harness.

Expectations are comments inside the case files (run with --rust / --both to use the Rust engine in rowan-rs/):

    r["role"]                 #@ possibly role
    r["rolle"]                #@ missing rolle ; none tenant
    d = build()               #@row d: present={id,role} definite={id} closed
    #@schema read_csv@L1: always={qty,price} sometimes={email}

Tiers: ok|proven, likely, possibly, missing, likely-missing, input, datakey, none (no verdict for
that key at this line), any (some verdict for the key).  Keys are bare words or Python literals.
A verdict matches a line if it was recorded at that line or while a call made from that line was active.
"""
from __future__ import annotations

import ast
import glob
import json
import os
import re
import subprocess
import sys
import traceback
from typing import Any, Dict, List, Tuple

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

from rowan.domain import Inv, key_str  # noqa: E402
from rowan.interp import Analyzer  # noqa: E402
from rowan.report import dedupe, format_text, schema_rows  # noqa: E402

TIER_ALIASES = {"ok": "proven", "proven": "proven", "likely": "likely", "possibly": "possibly",
                "missing": "missing", "likely-missing": "likely-missing", "input": "input",
                "datakey": "datakey", "none": "none", "any": "any", "map": "none"}


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


RUST_BIN = os.environ.get("ROWAN_RS", os.path.join(os.path.dirname(HERE), "rowan-rs", "target", "release", "rowan-rs"))
if not os.path.exists(RUST_BIN):
    RUST_BIN = os.path.join(os.path.dirname(HERE), "rowan-rs", "target", "debug", "rowan-rs")


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
    if spec.get("closed") and not row["closed"]:
        problems.append(f"expected closed, got {row.get('text', row)}")
    if spec.get("open") and row["closed"]:
        problems.append(f"expected open, got {row.get('text', row)}")
    return "; ".join(problems)


def run_case_rust(path: str, verbose: bool = False) -> Tuple[bool, List[str]]:
    with open(path, encoding="utf-8") as f:
        src = f.read()
    verdict_exp, row_exp, schema_exp = collect_expectations(src)
    try:
        cmd = [RUST_BIN, path, "--golden"] + (["--compat"] if os.environ.get("ROWAN_COMPAT") else [])
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    except Exception as e:  # noqa: BLE001
        return False, [f"could not run {RUST_BIN}: {e}"]
    if out.returncode != 0:
        return False, [f"rowan-rs failed (exit {out.returncode}): {out.stderr.strip()[-2000:]}"]
    try:
        res = json.loads(out.stdout)
    except json.JSONDecodeError as e:
        return False, [f"bad JSON from rowan-rs: {e}: {out.stdout[:200]}"]
    row_checks = []
    for rc in res["rows"]:
        for name, spec in row_exp.get(rc["line"], []):
            if name != rc["name"]:
                continue
            if rc["row"] is None:
                row_checks.append((rc["line"], name, spec, "no record object"))
            else:
                row_checks.append((rc["line"], name, spec, row_problems(rc["row"], spec)))
    seen_rows = {(rc["line"], rc["name"]) for rc in res["rows"]}
    for ln, specs in row_exp.items():
        for name, spec in specs:
            if (ln, name) not in seen_rows:
                row_checks.append((ln, name, spec, "statement never executed"))
    failures = check_results(verdict_exp, row_checks, schema_exp, res["verdicts"], res["schemas"])
    if verbose or failures:
        print(f"\n===== {os.path.basename(path)} [rust] =====")
        print(res["report"])
    return not failures, failures


def run_case(path: str, verbose: bool = False) -> Tuple[bool, List[str]]:
    with open(path, encoding="utf-8") as f:
        src = f.read()
    verdict_exp, row_exp, schema_exp = collect_expectations(src)
    failures: List[str] = []
    row_checks: List[Tuple[int, str, Dict[str, Any], str]] = []

    main_nodes: set = set()

    def on_stmt(stmt, state, an):
        if not main_nodes:
            main_nodes.update(id(n) for n in ast.walk(an.tree))
        if id(stmt) not in main_nodes:
            return
        for ln in range(stmt.lineno, (getattr(stmt, "end_lineno", stmt.lineno) or stmt.lineno) + 1):
            for name, spec in row_exp.get(ln, []):
                if ln != stmt.lineno:
                    continue
                fake = ast.Name(id=name, ctx=ast.Load())
                v = an.load_name(name, fake, state)
                rows = []
                for oid in sorted(v.pts):
                    o = an.objects[oid]
                    ns = "attr" if o.kind in ("instance", "namespace") else "item"
                    rows.append(state.row(oid, ns))
                if not rows:
                    row_checks.append((ln, name, spec, "no record object"))
                    continue
                from rowan.domain import join_rows
                row = join_rows(rows)
                problems = []
                if "present" in spec and set(row.present) != spec["present"]:
                    problems.append(f"present={set(row.present)} want {spec['present']}")
                if "definite" in spec and set(row.definite) != spec["definite"]:
                    problems.append(f"definite={set(row.definite)} want {spec['definite']}")
                if "absent" in spec and not spec["absent"] <= set(row.absent):
                    problems.append(f"absent={set(row.absent)} want ⊇ {spec['absent']}")
                if spec.get("closed") and not row.closed:
                    problems.append(f"expected closed, got {row}")
                if spec.get("open") and row.closed:
                    problems.append(f"expected open, got {row}")
                row_checks.append((ln, name, spec, "; ".join(problems)))

    an = Analyzer(src, os.path.basename(path), [os.path.dirname(path)], on_stmt=on_stmt)
    try:
        an.run()
    except Exception:
        return False, [traceback.format_exc()]
    vs = dedupe(an.verdicts)
    for ln, exps in sorted(verdict_exp.items()):
        here = [v for v in vs if v.line == ln or ln in v.chain]
        for tier, key in exps:
            if tier == "none" and key is None:
                bad = [v for v in here if v.tier not in ("proven",)]
                if bad:
                    failures.append(f"L{ln}: expected no diagnostics, got " + ", ".join(f"{v.tier} {key_str(v.key)}" for v in bad))
                continue
            matches = [v for v in here if key_str(v.key) == key_str(key) or (isinstance(key, str) and str(v.key).startswith(key))]
            if tier == "none":
                if matches:
                    failures.append(f"L{ln}: expected no verdict for {key!r}, got " + ", ".join(v.tier for v in matches))
                continue
            if tier == "any":
                if not matches:
                    failures.append(f"L{ln}: expected some verdict for {key!r}, got none")
                continue
            if not matches:
                got = ", ".join(f"{v.tier} {key_str(v.key)}" for v in here) or "nothing"
                failures.append(f"L{ln}: expected {tier} {key!r}; got {got}")
            elif not any(v.tier == tier for v in matches):
                failures.append(f"L{ln}: expected {tier} {key!r}; got " + ", ".join(sorted({v.tier for v in matches})))
    for ln, name, spec, problem in row_checks:
        if problem:
            failures.append(f"L{ln} row {name}: {problem}")
    sch = schema_rows(an)
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
    if verbose or failures:
        print(f"\n===== {os.path.basename(path)} =====")
        print(format_text(an))
    return not failures, failures


def main(argv: List[str]) -> int:
    verbose = "-v" in argv
    engine = "python"
    for a in argv:
        if a.startswith("--engine="):
            engine = a.split("=", 1)[1]
    if "--rust" in argv:
        engine = "rust"
    if "--both" in argv:
        engine = "both"
    args = [a for a in argv if not a.startswith("-")]
    paths = args or sorted(glob.glob(os.path.join(HERE, "cases", "*.py"))
                           + glob.glob(os.path.join(HERE, "cases", "*", "*.py")))
    paths = [p for p in paths if not os.path.basename(p).startswith("helpers")]
    ok_all = True
    results = []
    engines = ["python", "rust"] if engine == "both" else [engine]
    for p in paths:
        with open(p, encoding="utf-8") as f:
            head = f.read(200)
        m = re.search(r"#@engine\s+(\w+)", head)
        only = m.group(1) if m else None
        for eng in engines:
            if only is not None and (only != eng or (only == "rust" and os.environ.get("ROWAN_COMPAT"))):
                continue   # a case for one engine only (and `--compat` is the prototype's behaviour)
            ok, fails = (run_case_rust if eng == "rust" else run_case)(p, verbose)
            results.append((os.path.basename(p) + ("" if len(engines) == 1 else f" [{eng}]"), ok, fails))
            ok_all &= ok
    print("\n===== summary =====")
    npass = sum(1 for _n, ok, _f in results if ok)
    for name, ok, fails in results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}")
        for f in fails:
            print(f"        - {f}")
    print(f"  {npass}/{len(results)} passed ({engine})")
    return 0 if ok_all else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
