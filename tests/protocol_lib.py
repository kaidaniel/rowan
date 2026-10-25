"""What the protocol enumerators share (tests/gen_protocol.py, with_protocol.py, iter_protocol.py):
programs as Python text, CPython's runs of them, rowan's claims, and the check that the claims
cover every run.

A *program* is a list of module-level lines: the functions it defines and its entry function
`pN` (`Prog.lines`, written for N = 0; `{N}` in them is replaced). Observation lines are
assignments to a global whose name starts with `O` of a name's value (`OG = G`: no call, which
the tracer would read inside of): CPython's value there is read by
a line tracer once the line ran; rowan's claim is `#@general NAME` on that line (its main run's
rows and, where present, its rows for any call). Each program runs once at module level,
`run(pN)` (any raise caught), followed by its end observations (module-level lines `OG = G`…).

Invariant (`check`): at every observation point, every value a CPython run reaches is among
rowan's claims: the main run's rows (a point with none is a claim that it is not reached), and
the rows for any call where there are any.
"""
from __future__ import annotations

import argparse
import json
import os
import random
import re
import subprocess
import sys
import tempfile
import time
import warnings
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST_BIN = os.environ.get("ROWAN_BIN", str(ROOT / "rowan" / "target" / "release" / "rowan"))
GUARD = ROOT / ".guard" / "run"
OBS_RE = re.compile(r"^\s*(O[A-Z0-9]*) = ")

INPUTS = [(c1, c2, b) for c1 in (False, True) for c2 in (False, True) for b in ("", "K", "V")]

PRELUDE = '''import contextlib
import os

c1 = os.environ.get("C1") == "1"
c2 = os.environ.get("C2") == "1"
B = os.environ.get("B", "")
{globals}


class Stop(BaseException):
    pass


def boom():
    if B == "K":
        raise KeyError("k")
    if B == "V":
        raise ValueError("v")


def run(f):
    global {reset_names}
{reset}
    try:
        f()
    except BaseException:
        pass
'''


@dataclass
class Prog:
    lines: list  # module-level lines (functions), `{N}` for the program's number
    end: list = field(default_factory=list)  # module-level lines after `run(pN)`
    desc: str = ""

    def text(self, n: int = 0) -> list:
        return [ln.replace("{N}", str(n)) for ln in self.lines]


def module(progs: list, names: list) -> tuple[str, dict]:
    """One module of the programs; {line number: (program, point, name)} for its observation
    lines (point: the line's index in the program's own text, or ("end", i))."""
    g = "\n".join(f"{x} = 0" for x in names)
    reset = "\n".join(f"    {x} = 0" for x in names)
    lines = PRELUDE.format(globals=g, reset_names=", ".join(names), reset=reset).split("\n")
    where: dict = {}
    for n, p in enumerate(progs):
        lines.append("")
        base = len(lines)
        for i, ln in enumerate(p.text(n)):
            lines.append(ln)
            m = OBS_RE.match(ln)
            if m:
                where[base + i + 1] = (n, i, m.group(1))
        lines.append(f"run(p{n})")
        for i, ln in enumerate(p.end):
            lines.append(ln.replace("{N}", str(n)))
            m = OBS_RE.match(ln)
            if m:
                where[len(lines)] = (n, ("end", i), m.group(1))
    return "\n".join(lines) + "\n", where


def concrete(text: str, where: dict) -> dict:
    """{(program, point): values} over every input, from CPython's runs: a line tracer reads
    the observed global once its line ran (at the next event)."""
    seen: dict = defaultdict(set)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        code = compile(text, "<protocol>", "exec")
    old_hook = sys.unraisablehook
    sys.unraisablehook = lambda u: None  # (a raise in a finally run by a generator's collection)
    try:
        for c1, c2, b in INPUTS:
            os.environ.update(C1="1" if c1 else "0", C2="1" if c2 else "0", B=b)
            pending: list = []
            g: dict = {"__name__": "__protocol__"}

            def flush():
                while pending:
                    ln = pending.pop()
                    n, pt, name = where[ln]
                    seen[(n, pt)].add(repr(g.get(name)))

            def tracer(frame, event, arg):
                if frame.f_code.co_filename != "<protocol>":
                    return None
                flush()
                if event == "line" and frame.f_lineno in where:
                    pending.append(frame.f_lineno)
                return tracer

            sys.settrace(tracer)
            try:
                exec(code, g)
            finally:
                sys.settrace(None)
            flush()
    finally:
        sys.unraisablehook = old_hook
    return seen


def rowan_claims(text: str, where: dict, limits: str | None = None, timeout: int = 300) -> tuple[dict, dict, str]:
    """({(program, point): (literals, unknown)} of the main run, the same for any call, stderr)."""
    lines = text.split("\n")
    for ln, (_, _, name) in where.items():
        lines[ln - 1] += f"  #@general {name}"
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "protocol_batch.py"
        p.write_text("\n".join(lines))
        cmd = [RUST_BIN, str(p), "--golden"]
        # (the guard is the owner's Mac's; CI's macOS runners have none)
        if sys.platform == "darwin" and GUARD.exists():
            cmd = [str(GUARD), "-t", str(timeout), *cmd]
        env = dict(os.environ)
        if limits:
            env["ROWAN_LIMITS"] = limits
        r = subprocess.run(cmd, capture_output=True, text=True, stdin=subprocess.DEVNULL, env=env)
    try:
        doc = json.loads(r.stdout)
    except json.JSONDecodeError:
        return None, None, r.stderr[-2000:]
    main: dict = {}
    gen: dict = {}
    for rc in doc["rows"]:
        if rc.get("file", "") or rc["line"] not in where:
            continue
        n, pt, _ = where[rc["line"]]
        v = rc["value"]
        tgt = gen if rc.get("general") else main
        lits, unk = tgt.get((n, pt), (set(), False))
        unk = unk or v["unknown"] or bool(v.get("objects"))
        tgt[(n, pt)] = (lits | set(v["lits"]), unk)
    # (no claim at any observation line: a rowan without `#@general` rows, not one claiming
    # every line unreached)
    if not main and not gen:
        return None, None, "no rows: this rowan writes no claims for #@general lines"
    return main, gen, r.stderr


@dataclass
class Tally:
    programs: int = 0
    points: int = 0
    missed: int = 0  # points where a CPython value is not among the claims
    exact: int = 0  # points whose main-run claims are exactly CPython's values
    crashed: int = 0  # batches rowan gave no document for
    bad_programs: list = field(default_factory=list)
    seconds: float = 0.0


def check(progs: list, names: list, tally: Tally, show: int = 5, verbose: bool = False, limits: str | None = None,
          batch: int = 150, label: str = "") -> list:
    """Runs CPython and rowan on the programs (batched) and counts the points where rowan's
    claims miss a CPython run; returns the indices of the programs with a miss."""
    bad = []
    t0 = time.time()
    for start in range(0, len(progs), batch):
        part = progs[start:start + batch]
        text, where = module(part, names)
        conc = concrete(text, where)
        main, gen, err = rowan_claims(text, where, limits)
        tally.programs += len(part)
        if main is None:
            tally.crashed += 1
            print(f"ROWAN FAILED on a batch {label}:\n{err}")
            continue
        badhere = set()
        for (n, pt), xs in conc.items():
            tally.points += 1
            missed = set()
            # (module-level lines have rows for any call only)
            for claims, need in ((main, (n, pt) not in gen), (gen, False)):
                if (n, pt) not in claims and not need:
                    continue
                lits, unk = claims.get((n, pt), (set(), False))
                if not unk:
                    missed |= {x for x in xs if x not in lits}
            mlits, munk = main.get((n, pt)) or gen.get((n, pt)) or (set(), False)
            if missed:
                tally.missed += 1
                if n not in badhere:
                    badhere.add(n)
                    if len(tally.bad_programs) < show or verbose:
                        print(f"ROWAN MISSES {sorted(missed)} at {where_text(part[n], pt)} "
                              f"(main {sorted(mlits)}{' | ?' if munk else ''}; any call {fmt(gen.get((n, pt)))}){' ' + label if label else ''}\n"
                              + show_prog(part[n], pt) + "\n")
                    tally.bad_programs.append(start + n)
            elif not munk and xs == mlits:
                tally.exact += 1
        bad += [start + n for n in sorted(badhere)]
    tally.seconds += time.time() - t0
    return bad


def fmt(c) -> str:
    if c is None:
        return "none"
    lits, unk = c
    return f"{sorted(lits)}{' | ?' if unk else ''}"


def where_text(p: Prog, pt) -> str:
    if isinstance(pt, tuple):
        return f"the end ({p.end[pt[1]].strip()})"
    return f"line {pt + 1} ({p.text()[pt].strip()})"


def show_prog(p: Prog, pt=None) -> str:
    out = []
    for i, ln in enumerate(p.text()):
        out.append(ln + ("    # <-" if i == pt else ""))
    out.append("run(p0)")
    for i, ln in enumerate(p.end):
        out.append(ln + ("    # <-" if pt == ("end", i) else ""))
    return "\n".join(out)


def report(tally: Tally, what: str) -> str:
    return (f"{what}: {tally.programs} programs, {tally.points} points checked, {tally.missed} with a run rowan's claims miss "
            f"({len(set(tally.bad_programs))} programs), {tally.exact} exact; rowan failed on {tally.crashed} batches; {tally.seconds:.1f} s")


def cli(programs, names: list, quick: int = 2000, gaps: tuple = (), limit_sets: tuple = (None,)) -> None:
    """The command line every enumerator shares: CPython on every program, rowan on a sample
    (`--rowan N`, default `quick`) or on all (`--all`), under each of `limit_sets` (ROWAN_LIMITS;
    `--limits` names one; `programs(limits)`, when it takes them, makes the programs for each);
    exit 1 when rowan's claims miss a run.
    Programs whose description names one of `gaps` (known misses, listed in AGENTS.md) are
    checked and counted apart, and fail the run only with `--strict`."""
    ap = argparse.ArgumentParser()
    ap.add_argument("--rowan", type=int, default=quick, help="programs rowan runs on (a sample)")
    ap.add_argument("--all", action="store_true", help="rowan on every program")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--limits", help="ROWAN_LIMITS for rowan's runs")
    ap.add_argument("--strict", action="store_true", help="the known gaps fail the run too")
    ap.add_argument("-v", action="store_true")
    a = ap.parse_args()
    progs = list(programs())
    print(f"{len(progs)} programs, {len(INPUTS)} inputs each")
    rng = random.Random(a.seed)
    idx = sorted(rng.sample(range(len(progs)), min(a.rowan, len(progs))))
    pick = progs if a.all else [progs[i] for i in idx]
    gap = lambda p: any(g in p.desc.split() for g in gaps)
    failed = False
    for limits in ([a.limits] if a.limits else limit_sets):
        at = f" (ROWAN_LIMITS={limits})" if limits else ""
        if programs.__code__.co_argcount:
            # (programs that depend on the limits: the same ones, observed differently)
            every = list(programs(limits))
            pick = every if a.all else [every[i] for i in idx]
        t, tg = Tally(), Tally()
        check([p for p in pick if not gap(p)], names, t, verbose=a.v, limits=limits, label=at.strip())
        print(report(t, "rowan" + at))
        if gaps:
            check([p for p in pick if gap(p)], names, tg, show=0, limits=limits, label=at.strip())
            print(report(tg, f"rowan on the known gaps ({', '.join(gaps)}){at}"))
        failed |= bool(t.missed or t.crashed or (a.strict and (tg.missed or tg.crashed)))
    sys.exit(1 if failed else 0)
