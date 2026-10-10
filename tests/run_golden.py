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
Values: `#@value NAME: exactly={12}` (the value is one of these literals and nothing else),
`#@value NAME: unknown` (rowan claims no specific value), `#@value NAME: includes={1,100}`
(floor: unknown, or a set holding these); several names on one line separated by `;`.
Row checks beyond present/definite/closed/open: `possible={..}` (each key is present, or the
row is open) and `definite_within={..}` (every definite key is listed): soundness floors for
cases whose precise answer is a `#@todo`.  Keys are bare words or Python literals.
Tensor shapes (`#@shape`, `#@dim`, `#@requires`, `#@shape-error`): see tests/shapes.py.
A verdict matches a line if it was recorded at that line or while a call made from that line was
active.  A `#@` directive must be the last thing on its line (the key runs to its end).

    python tests/run_golden.py [-v] [case…]
    python tests/run_golden.py --pending [--prune]   # the pending torch / pandas cases (prune: drop passing ones)
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

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from shapes import ShapeExpectations, directive_lines  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

TODO_COUNT = [0]
SHAPES = [ShapeExpectations()]  # `#@shape` / `#@dim` / `#@requires` / `#@shape-error` (tests/shapes.py)
VALUE_EXP: Dict[int, List[Tuple[str, Dict[str, Any]]]] = {}  # line -> [(name, spec)] for `#@value`  # `#@todo` directives seen: expectations the engine does not meet yet

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
    # top-level commas only: tuple keys (`("y", "max")`, MultiIndex columns) stay whole
    parts, depth, cur = [], 0, ""
    for ch in s:
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    parts.append(cur)
    return {parse_key(t) for t in parts if t.strip()}


def collect_expectations(src: str):
    verdict_exp: Dict[int, List[Tuple[str, Any]]] = {}
    row_exp: Dict[int, List[Tuple[str, Dict[str, Any]]]] = {}
    VALUE_EXP.clear()
    SHAPES[0] = ShapeExpectations()
    SHAPES[0].eff = directive_lines(src)
    schema_exp: List[Tuple[str, Dict[str, Any]]] = []
    eff = SHAPES[0].eff
    for i0, line in enumerate(src.splitlines(), 1):
        # a line holding only a directive comment applies to the code line above it
        i = eff[i0] if i0 < len(eff) else i0
        if "#@todo" in line:
            TODO_COUNT[0] += 1
            if os.environ.get("ROWAN_TODO") != "check":
                continue
            # ROWAN_TODO=check: the open expectations are checked like the others
            line = re.sub(r"#@todo\s+row\b", "#@row", line)
            line = re.sub(r"#@todo\s+value\b", "#@value", line)
            line = re.sub(r"#@todo\s+schema\b", "#@schema", line)
            line = re.sub(r"#@todo\s+(shape|dim|requires)\b", r"#@\1", line)
            line = re.sub(r"#@todo\s+", "#@ ", line)
        if SHAPES[0].parse_line(i0, line):
            continue
        m = re.search(r"#@value\s+(.*)$", line)
        if m:
            for clause in m.group(1).split(";"):
                cm = re.match(r"\s*(\w+)\s*:\s*(.*)$", clause)
                if not cm:
                    raise ValueError(f"line {i}: bad #@value clause {clause!r}")
                spec: Dict[str, Any] = {}
                for part in re.findall(r"(\w+)=(\{[^}]*\})", cm.group(2)):
                    spec[part[0]] = parse_set(part[1])
                if re.search(r"\bunknown\b", re.sub(r"\{[^}]*\}", "", cm.group(2))):
                    spec["unknown"] = True
                VALUE_EXP.setdefault(i, []).append((cm.group(1), spec))
            continue
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
                bad = [v for v in here if v["tier"] not in ("proven",) and not v.get("guarded")]
                if bad:
                    failures.append(f"L{ln}: expected no diagnostics, got " + ", ".join(f"{v['tier']} {v['key']}" for v in bad))
                continue
            matches = [v for v in here if v["key"] == key_str(key) or (isinstance(key, str) and v["key_str"].startswith(key))]
            if tier == "none":
                # `none` = no diagnostic: a verdict on an access the program guards is none
                matches = [v for v in matches if not v.get("guarded")]
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
            # `possibly` over several calls: per-call verdicts that differ (missing in one call,
            # present in another) say the same, more precisely
            if tier == "possibly" and any(v["tier"] in ("missing", "likely-missing") for v in matches) and any(v["tier"] in ("proven", "likely", "input") for v in matches):
                continue
            # (the oracle's reading: the line raised for k in some runs and not in others, where
            # the access read another key: rowan's per-key verdicts say k fails, the other key not)
            if tier == "possibly" and matches and all(v["tier"] in ("missing", "likely-missing") for v in matches):
                cols = {v.get("col") for v in matches}
                if any(v["key"] != key_str(key) and v.get("col") in cols and v["tier"] in ("proven", "likely", "input") for v in here):
                    continue
            # and the converse: per-run tiers that differ (`ok k ; missing k`) joined into one
            # verdict over the runs are `possibly`
            if tier != "possibly" and any(v["tier"] == "possibly" for v in matches):
                tiers_here = {t for t, k in exps if k == key}
                if tiers_here & {"missing", "likely-missing"} and tiers_here & {"proven", "ok", "likely", "input", "possibly"}:
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
        # the source itself (a module's sources may carry a prefix), not a path below it
        hits = {k: v for k, v in sch.items() if k == label or (k.endswith(label) and k[-len(label) - 1] in ": ")}
        if spec.get("none"):
            keys = {k for e in hits.values() for k in e}
            if keys:
                failures.append(f"schema {label}: expected none, got {sorted(keys)}")
            continue
        if not hits:
            # nothing required is what `only={}` (with no always / sometimes) says
            if spec.get("always") or spec.get("sometimes") or "only" not in spec:
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
            allowed = set(spec["only"]) | set(spec.get("sometimes", set())) | set(spec.get("always", set()))
            extra = allk - {key_str(k) for k in allowed}
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
    if row["closed"]:
        # a closed row has nothing outside `present`
        absent |= {key_str(k) for k in spec.get("absent", ())} - present
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


def case_files(path: str) -> List[Tuple[str, str]]:
    """(path relative to the entry's directory, "" for the entry; source) of the files whose
    directives a case checks: the entry and, in a package case (a directory whose name shares
    the entry's prefix: `k94_pkg/k94_modules.py`), every module under that directory."""
    with open(path, encoding="utf-8") as f:
        out = [("", f.read())]
    d = os.path.dirname(os.path.abspath(path))
    prefix = os.path.basename(path).split("_")[0]
    if prefix and os.path.basename(d).startswith(prefix + "_"):
        for p in sorted(glob.glob(os.path.join(d, "**", "*.py"), recursive=True)):
            if os.path.abspath(p) != os.path.abspath(path):
                with open(p, encoding="utf-8") as f:
                    out.append((os.path.relpath(p, d), f.read()))
    return out


def run_case(path: str, verbose: bool = False) -> Tuple[bool, List[str]]:
    with open(path, encoding="utf-8") as f:
        src = f.read()
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
    failures: List[str] = []
    for rel, fsrc in case_files(path):
        fails = check_file(fsrc, rel, res)
        failures += [f"{rel}: {f}" if rel else f for f in fails]
    if verbose or failures:
        print(f"\n===== {os.path.basename(path)} =====")
        print(res["report"])
    return not failures, failures


def check_file(src: str, rel: str, res: Dict[str, Any]) -> List[str]:
    """Check the directives of one file of a case (`rel`: "" for the entry) against rowan's
    golden output for the whole case."""
    verdict_exp, row_exp, schema_exp = collect_expectations(src)
    rows_here = [rc for rc in res["rows"] if rc.get("file", "") == rel]
    # verdicts in this file (or reached through a call on one of its lines: the chain)
    vs_here = [v for v in res["verdicts"] if (v.get("file") or "") == rel or v.get("chain")]
    # a line can run several times (a function called from several places, or analysed as an
    # entry point with open parameters): its row is the join over the runs
    joined: Dict[Tuple[int, str], Any] = {}
    for rc in rows_here:
        key = (rc["line"], rc["name"])
        r = rc["row"]
        if key not in joined:
            joined[key] = None if r is None else dict(r, present=list(r["present"]), definite=list(r["definite"]), absent=list(r["absent"]))
        elif r is not None:
            j = joined[key]
            if j is None:
                joined[key] = dict(r)
                continue
            # (a closed row lacks every key outside its present ones: absent there too)
            ja = set(j["absent"]) if not j["closed"] else None
            ra = set(r["absent"]) if not r["closed"] else None
            if ja is None and ra is None:
                absent = set(j["absent"]) & set(r["absent"])
            elif ja is None:
                absent = ra - set(j["present"])
            elif ra is None:
                absent = ja - set(r["present"])
            else:
                absent = ja & ra
            j["present"] = sorted(set(j["present"]) | set(r["present"]))
            j["definite"] = sorted(set(j["definite"]) & set(r["definite"]))
            j["absent"] = sorted(absent)
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
    # values: joined over the runs of the line like rows
    values: Dict[Tuple[int, str], Dict[str, Any]] = {}
    for rc in rows_here:
        v = rc.get("value")
        if v is None:
            continue
        j = values.setdefault((rc["line"], rc["name"]), {"lits": set(), "unknown": False, "objects": set()})
        j["lits"] |= set(v["lits"])
        j["unknown"] = j["unknown"] or v["unknown"]
        j["objects"] |= set(v["objects"])
    for ln, specs in VALUE_EXP.items():
        for name, spec in specs:
            got = values.get((ln, name))
            if got is None:
                row_checks.append((ln, name, spec, "statement never executed"))
                continue
            desc = f"lits={sorted(got['lits'])} unknown={got['unknown']} objects={sorted(got['objects'])}"
            prob = ""
            if "exactly" in spec:
                want = {key_str(k) for k in spec["exactly"]}
                if got["unknown"] or got["objects"] or got["lits"] != want:
                    prob = f"value {desc}, want exactly {sorted(want)}"
            if spec.get("unknown") and not got["unknown"]:
                prob = f"value {desc}, but it may be anything: rowan must not claim a specific value"
            if "includes" in spec and not got["unknown"]:
                lost = {key_str(k) for k in spec["includes"]} - got["lits"]
                if lost:
                    prob = f"value {desc} leaves out {sorted(lost)}"
            row_checks.append((ln, f"value of {name}", spec, prob))
    failures = check_results(verdict_exp, row_checks, schema_exp, [v if (v.get("file") or "") == rel else dict(v, line=-1) for v in vs_here], res["schemas"])
    if not rel:
        failures += SHAPES[0].check(res)
    return failures


SUITES = ("torch", "pandas", "pydantic", "fastapi", "stdlib", "apps", "numpy")


def main(argv: List[str]) -> int:
    verbose = "-v" in argv
    args = [a for a in argv if not a.startswith("-")]
    pending_only = "--pending" in argv
    paths = args or sorted(glob.glob(os.path.join(HERE, "cases", "*.py"))
                           + glob.glob(os.path.join(HERE, "cases", "*", "*.py")))
    # suites (tests/torch/, tests/pandas/: {small,models}/*.py): every file is a case, except
    # the ones listed in the suite's PENDING (written, not yet expected to pass; `--pending`
    # runs only those)
    suites = [os.path.join(HERE, d) for d in SUITES if os.path.isdir(os.path.join(HERE, d))]
    pending: Dict[str, str] = {}  # case path -> suite dir
    for sd in suites:
        pf = os.path.join(sd, "PENDING")
        if os.path.exists(pf):
            with open(pf) as f:
                for ln in f:
                    name = ln.split("#")[0].strip()
                    if name:
                        pending[os.path.join(sd, name)] = sd
    # a suite case is a file in `small/` / `models/`, or the entry of a package case below them
    # (`apps/large/a01_shop/a01_main.py`)
    suite_cases = sorted(p for sd in suites for p in glob.glob(os.path.join(sd, "*", "*.py")))
    for sd in suites:
        for d in sorted(glob.glob(os.path.join(sd, "*", "*", ""))):
            prefix = os.path.basename(os.path.dirname(d)).split("_")[0]
            suite_cases += sorted(glob.glob(os.path.join(d, prefix + "_*.py")))
    if not args:
        if pending_only:
            paths = [p for p in suite_cases if p in pending]
        else:
            paths += [p for p in suite_cases if p not in pending]
    # in a package directory only the entry module is a case (its name shares the directory's
    # prefix: `k94_pkg/k94_modules.py`); the other files are modules it imports
    def is_case(p):
        d = os.path.basename(os.path.dirname(p))
        if d == "cases" or os.path.dirname(os.path.dirname(p)) in suites or p in suite_cases:
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
    if pending_only and "--prune" in argv:
        passed = {p for p, (_n, ok, _f) in zip(paths, results) if ok}
        for sd in suites:
            pf = os.path.join(sd, "PENDING")
            if not os.path.exists(pf):
                continue
            with open(pf) as f:
                lines = f.read().splitlines()
            with open(pf, "w") as f:
                f.write("\n".join(ln for ln in lines if os.path.join(sd, ln.split("#")[0].strip()) not in passed or not ln.split("#")[0].strip()) + "\n")
        print(f"  pruned {len(passed)} passing cases from PENDING")
    print(f"  {npass}/{len(results)} passed" + (f"  ({TODO_COUNT[0]} #@todo expectations not checked)" if TODO_COUNT[0] else "")
          + (f"  ({len(pending)} suite cases pending)" if pending and not args and not pending_only else ""))
    return 0 if ok_all else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
