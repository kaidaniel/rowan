"""An executable model of rowan's try / finally raise routing, checked exhaustively against CPython.

The protocol (AGENTS.md, Statements; rowan/src/interp/tries.rs): a running `try` records the
state at every point of its body that may raise, joined per exception class; each handler starts
from the raises it may catch (the first handler that surely catches a class takes it), the rest go
on through the `finally` to the enclosing try; raises every handler around surely misses leave
the program, recorded as one whatever their class, and the `finally` runs once from their states
joined (`leaves_program`); an `else` runs from the body's normal end and its raises pass the
handlers by; a `finally` runs on every way out (normal end, return, raise), and one that returns
or raises replaces the way out.

The model (`Model`) runs these rules on sets of program states (the global trace `X`: each
marker `X = X * 10 + k`), as rowan's abstract states would hold them, and records the states at
observation points: each handler's and finally's start, after each try, and at the program's
end. The concrete semantics is CPython itself: each program runs on every input (two conditions,
and `boom()` raising KeyError, ValueError or nothing).

Invariant: at every observation point, every state a CPython run reaches is among the model's
(the model covers the runs); and so are rowan's claims (`#@general` at each point, the value of
`X` there).

    uv run --no-project python tests/try_protocol.py [--rowan N] [--seed S] [-v]

explores every program of the bounded grammar (`programs`) in the model against CPython, then
runs rowan (through .guard/run when on macOS) on N of them (default 600; and on every program
the model found a counterexample in), batched 150 to a file, and checks its claims the same way.
"""
from __future__ import annotations

import argparse
import itertools
import json
import os
import random
import subprocess
import sys
import tempfile
import warnings
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST_BIN = os.environ.get("ROWAN_BIN", str(ROOT / "rowan" / "target" / "release" / "rowan"))
GUARD = ROOT / ".guard" / "run"

# ---------------------------------------------------------------- the classes
# (Stop is a BaseException: `except Exception` misses it)
MRO = {
    "KeyError": ["KeyError", "LookupError", "Exception", "BaseException"],
    "ValueError": ["ValueError", "Exception", "BaseException"],
    "Stop": ["Stop", "BaseException"],
}
HANDLERS = ["KeyError", "LookupError", "Exception", None]  # None: a bare `except:`


def catches(h: str | None, cls: str) -> bool:
    return h is None or h in MRO[cls]


# ---------------------------------------------------------------- the grammar
# A statement: ("mark", k) | ("raise", cls) | ("boom",) | ("return",) | ("reraise",) |
# ("if", i, [stmts]) | ("try", body, [(handler, body)], orelse, finalbody)


def leaves(in_handler: bool):
    out = [("mark", 1), ("raise", "KeyError"), ("raise", "Stop"), ("boom",), ("return",), ("if", 1, [("raise", "ValueError")]), ("if", 2, [("return",)])]
    return out + ([("reraise",)] if in_handler else [])


def blocks(in_handler: bool):
    """Bodies: one statement or two."""
    ls = leaves(in_handler)
    yield from ([s] for s in ls)
    for a in ls:
        for b in ls:
            if a[0] not in ("return", "reraise"):
                yield [a, b]


def handler_lists(small: bool):
    """None, one handler or two (the second's body a marker), in every order."""
    bodies = [[("mark", 3)], [("reraise",)]] if small else [[("mark", 3)], [("reraise",)], [("raise", "ValueError")], [("return",)]]
    yield []
    for h in HANDLERS:
        for b in bodies:
            yield [(h, b)]
    if small:
        return
    for h1, h2 in itertools.permutations(HANDLERS, 2):
        if h1 is None:
            continue  # (a bare `except:` comes last)
        for b1 in bodies[:2]:
            yield [(h1, b1), (h2, [("mark", 4)])]


ORELSES = [[], [("mark", 5)], [("raise", "ValueError")]]
FINALS = [[], [("mark", 6)], [("return",)], [("raise", "ValueError")]]


def tries(bodies, hlists, orelses, finals):
    for body in bodies:
        for hs in hlists:
            for orelse in orelses:
                for fin in finals:
                    if (hs or fin) and (hs or not orelse):
                        yield ("try", body, hs, orelse, fin)


def programs():
    """Every flat try (bodies of one or two statements, up to two handlers, else, finally),
    and every try around a marker and a small try (one statement, up to one handler, finally
    or not) with up to one handler of its own."""
    for t in tries(list(blocks(False)), list(handler_lists(False)), ORELSES, FINALS):
        yield [("mark", 7), t]
    small = list(tries([[s] for s in leaves(False)], list(handler_lists(True)), [[]], [[], [("mark", 6)]]))
    for inner in small:
        for t in tries([[("mark", 2), inner]], list(handler_lists(True)), [[]], FINALS):
            yield [("mark", 7), t]


# ---------------------------------------------------------------- the model
INPUTS = [(c1, c2, b) for c1 in (False, True) for c2 in (False, True) for b in ("", "K", "V")]


class Out:
    """The ways out of a block: normal end, returns, raises per class (sets of states)."""

    def __init__(self):
        self.normal: set = set()
        self.ret: set = set()
        self.raises: dict = defaultdict(set)

    def add(self, o: "Out", normal: bool = True):
        if normal:
            self.normal |= o.normal
        self.ret |= o.ret
        for c, s in o.raises.items():
            self.raises[c] |= s


class Model:
    """The protocol's rules on sets of (state, input) pairs: X and the input deciding the
    conditions (rowan's paths keep what a condition tested; the model keeps the input)."""

    def __init__(self):
        self.obs: dict = defaultdict(set)  # point -> X values

    def block(self, stmts, S: set, handled: str | None, around) -> Out:
        out = Out()
        cur = set(S)
        for st in stmts:
            if not cur:
                break
            o = self.stmt(st, cur, handled, around)
            out.add(o, normal=False)
            cur = o.normal
        out.normal = cur
        return out

    def stmt(self, st, S, handled, around) -> Out:
        o = Out()
        kind = st[0]
        if kind == "mark":
            o.normal = {(x * 10 + st[1], i) for x, i in S}
        elif kind == "raise":
            o.raises[st[1]] |= S
        elif kind == "reraise":
            o.raises[handled] |= S
        elif kind == "return":
            o.ret |= S
        elif kind == "boom":
            for x, i in S:
                b = INPUTS[i][2]
                if b == "K":
                    o.raises["KeyError"].add((x, i))
                elif b == "V":
                    o.raises["ValueError"].add((x, i))
                else:
                    o.normal.add((x, i))
        elif kind == "if":
            t = {(x, i) for x, i in S if INPUTS[i][st[1] - 1]}
            f = S - t
            ob = self.block(st[2], t, handled, around)
            o.add(ob)
            o.normal |= f
        elif kind == "try":
            o = self.try_(st, S, handled, around)
        return o

    def try_(self, st, S, handled, around) -> Out:
        _, body, hs, orelse, fin = st
        pid = id(st)
        inner_around = around + [[h for h, _ in hs]]
        b = self.block(body, S, handled, inner_around)
        out = Out()
        out.ret |= b.ret
        after_handlers = Out()
        for cls, states in b.raises.items():
            for h, hb in hs:
                if catches(h, cls):
                    self.obs[(pid, "handler", hs.index((h, hb)))] |= {x for x, _ in states}
                    after_handlers.add(self.block(hb, states, cls, around))
                    break
            else:
                out.raises[cls] |= states
        normal = set()
        if orelse:
            e = self.block(orelse, b.normal, handled, around)
            normal |= e.normal
            out.ret |= e.ret
            for c, s in e.raises.items():
                out.raises[c] |= s
        else:
            normal |= b.normal
        normal |= after_handlers.normal
        out.ret |= after_handlers.ret
        for c, s in after_handlers.raises.items():
            out.raises[c] |= s
        if not fin:
            out.normal = normal
            self.obs[(pid, "after")] |= {x for x, _ in out.normal}
            return out
        res = Out()
        # the finally on each way out: the normal end, the returns, and the raises — per class,
        # except those every handler around surely misses (they leave the program): one run from
        # their states joined, each class raised again from its end
        ways = [("normal", normal), ("ret", out.ret)]
        leaving = {c: s for c, s in out.raises.items() if not any(catches(h, c) for hs2 in around for h in hs2)}
        for c, s in out.raises.items():
            if c not in leaving:
                ways.append((("raise", c), s))
        if leaving:
            ways.append((("leave", tuple(sorted(leaving))), set().union(*leaving.values())))
        for way, s in ways:
            if not s:
                continue
            self.obs[(pid, "finally")] |= {x for x, _ in s}
            f = self.block(fin, s, handled, around)
            res.ret |= f.ret
            for c, r in f.raises.items():
                res.raises[c] |= r
            if way == "normal":
                res.normal |= f.normal
            elif way == "ret":
                res.ret |= f.normal
            elif way[0] == "raise":
                res.raises[way[1]] |= f.normal
            else:
                for c in way[1]:
                    res.raises[c] |= f.normal
        self.obs[(pid, "after")] |= {x for x, _ in res.normal}
        return res


# ---------------------------------------------------------------- Python text
def emit(stmts, ind: int, pts: list, out: list | None = None) -> list[str]:
    """Lines of `stmts` (appended to `out`); `pts` collects the observation points as (line
    index in `out`, point id)."""
    pad = "    " * ind
    out = [] if out is None else out
    for st in stmts:
        k = st[0]
        if k == "mark":
            out.append(f"{pad}X = X * 10 + {st[1]}")
        elif k == "raise":
            out.append(f"{pad}raise {st[1]}()")
        elif k == "reraise":
            out.append(f"{pad}raise")
        elif k == "return":
            out.append(f"{pad}return")
        elif k == "boom":
            out.append(f"{pad}boom()")
        elif k == "if":
            out.append(f"{pad}if c{st[1]}:")
            emit(st[2], ind + 1, pts, out)
        elif k == "try":
            _, body, hs, orelse, fin = st
            pid = id(st)
            out.append(f"{pad}try:")
            emit(body, ind + 1, pts, out)
            for n, (h, hb) in enumerate(hs):
                out.append(f"{pad}except {h}:" if h else f"{pad}except:")
                pts.append((len(out), (pid, "handler", n)))
                out.append(f"{pad}    OBS = X")
                emit(hb, ind + 1, pts, out)
            if orelse:
                out.append(f"{pad}else:")
                emit(orelse, ind + 1, pts, out)
            if fin:
                out.append(f"{pad}finally:")
                pts.append((len(out), (pid, "finally")))
                out.append(f"{pad}    OBS = X")
                emit(fin, ind + 1, pts, out)
            pts.append((len(out), (pid, "after")))
            out.append(f"{pad}OBS = X")
    return out


PRELUDE = '''import os

c1 = os.environ.get("C1") == "1"
c2 = os.environ.get("C2") == "1"
B = os.environ.get("B", "")
X = 0
OBS = 0


class Stop(BaseException):
    pass


def boom():
    if B == "K":
        raise KeyError("k")
    if B == "V":
        raise ValueError("v")


def run(f):
    global X
    X = 0
    try:
        f()
    except BaseException:
        pass
    return X
'''


def program_text(progs: list) -> tuple[str, dict]:
    """One module of programs p0, p1, … each run once at module level; the observation
    points: {line number: (program, point)}, the end of each its line `rN = run(pN)`."""
    lines = PRELUDE.split("\n")
    where: dict = {}
    for n, prog in enumerate(progs):
        pts: list = []
        body = emit(prog, 1, pts)
        lines.append("")
        lines.append(f"def p{n}():")
        lines.append("    global X, OBS")
        base = len(lines)
        lines += body
        for idx, pid in pts:
            where[base + idx + 1] = (n, pid)
        lines.append("")
        lines.append(f"r{n} = run(p{n})")
        where[len(lines)] = (n, "end")
    return "\n".join(lines) + "\n", where


def concrete(text: str, where: dict) -> dict:
    """{(program, point): X values} over every input, from CPython's runs: a line tracer reads
    OBS once its observation line ran (at the next event); the end is what `run` returned."""
    seen: dict = defaultdict(set)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # (`return` in a `finally`)
        code = compile(text, "<protocol>", "exec")
    lines = {ln for ln, (_, pid) in where.items() if pid != "end"}
    ends = {n for n, pid in where.values() if pid == "end"}
    for c1, c2, b in INPUTS:
        os.environ.update(C1="1" if c1 else "0", C2="1" if c2 else "0", B=b)
        pending: list = []
        g: dict = {"__name__": "__protocol__"}

        def flush():
            while pending:
                seen[where[pending.pop()]].add(g.get("OBS"))

        def tracer(frame, event, arg):
            if frame.f_code.co_filename != "<protocol>":
                return None
            flush()
            if event == "line" and frame.f_lineno in lines:
                pending.append(frame.f_lineno)
            return tracer

        sys.settrace(tracer)
        try:
            exec(code, g)
        finally:
            sys.settrace(None)
        flush()
        for n in ends:
            seen[(n, "end")].add(g[f"r{n}"])
    return seen


def model_obs(prog) -> dict:
    m = Model()
    # (nothing around the program catches: an uncaught raise leaves it, `leaves_program`)
    out = m.block(prog, {(0, i) for i in range(len(INPUTS))}, None, [])
    obs = {k: v for k, v in m.obs.items()}
    # the end: run() returns X whichever way p left
    end = {x for x, _ in out.normal | out.ret} | {x for s in out.raises.values() for x, _ in s}
    obs["end"] = end
    return obs


def rowan_claims(text: str, where: dict) -> dict:
    """{(program, point): (literals, unknown)} from `rowan --golden` on the module with
    `#@general OBS` (`X` at the ends) on each observation line."""
    lines = text.split("\n")
    for ln, (n, pid) in where.items():
        lines[ln - 1] += "  #@general " + ("X" if pid == "end" else "OBS")
        if pid == "end":
            lines[ln - 1] = lines[ln - 1].replace("#@general X", f"#@general r{n}")
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "protocol_batch.py"
        p.write_text("\n".join(lines))
        cmd = [RUST_BIN, str(p), "--golden"]
        if sys.platform == "darwin":
            cmd = [str(GUARD), "-t", "300", *cmd]
        r = subprocess.run(cmd, capture_output=True, text=True, stdin=subprocess.DEVNULL)
    doc = json.loads(r.stdout)
    claims: dict = {}
    for rc in doc["rows"]:
        if rc.get("file", "") or rc["line"] not in where:
            continue
        v = rc["value"]
        lits, unk = claims.get(where[rc["line"]], (set(), False))
        claims[where[rc["line"]]] = (lits | {int(x) for x in v["lits"] if x.lstrip("-").isdigit()}, unk or v["unknown"])
    return claims


def show(prog) -> str:
    pts: list = []
    return "\n".join(emit(prog, 0, pts))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rowan", type=int, default=600, help="programs rowan runs on (a sample, and the model's counterexamples)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("-v", action="store_true")
    a = ap.parse_args()
    progs = list(programs())
    print(f"{len(progs)} programs, {len(INPUTS)} inputs each")
    model_bad, exact, checked = [], 0, 0
    batch = 2000
    for start in range(0, len(progs), batch):
        part = progs[start:start + batch]
        text, where = program_text(part)
        conc = concrete(text, where)
        for n, prog in enumerate(part):
            obs = model_obs(prog)
            ok = True
            for (pn, pid), xs in conc.items():
                if pn != n:
                    continue
                checked += 1
                claim = obs.get(pid, set())
                if not xs <= claim:
                    ok = False
                    if a.v or len(model_bad) < 5:
                        print(f"MODEL MISSES at {pid if pid == 'end' else pid[1:]}: CPython {sorted(xs)}, model {sorted(claim)}\n{show(prog)}\n")
                elif xs == claim:
                    exact += 1
            if not ok:
                model_bad.append(start + n)
    print(f"model: {checked} points checked, {len(model_bad)} programs with a run the model misses, {exact} points exact")
    if a.rowan <= 0:
        return
    rng = random.Random(a.seed)
    pick = sorted(set(rng.sample(range(len(progs)), min(a.rowan, len(progs)))) | set(model_bad))
    rowan_bad, rchecked, rexact = 0, 0, 0
    agree = defaultdict(int)
    for start in range(0, len(pick), 150):
        part = [progs[i] for i in pick[start:start + 150]]
        text, where = program_text(part)
        conc = concrete(text, where)
        claims = rowan_claims(text, where)
        for (n, pid), xs in conc.items():
            rchecked += 1
            lits, unk = claims.get((n, pid), (set(), False))
            missed = {x for x in xs if x not in lits} if not unk else set()
            model_ok = pick[start + n] not in model_bad
            agree[(model_ok, not missed)] += 1
            if missed:
                rowan_bad += 1
                if rowan_bad <= 5 or a.v:
                    print(f"ROWAN MISSES {sorted(missed)} at {pid if pid == 'end' else pid[1:]} (claims {sorted(lits)}{' | ?' if unk else ''})\n{show(part[n])}\n")
            elif not unk and lits == xs:
                rexact += 1
    print(f"rowan: {len(pick)} programs, {rchecked} points checked, {rowan_bad} with a run rowan's claims miss, {rexact} exact")
    print("model covers / rowan covers:", {f"model {'ok' if m else 'misses'}, rowan {'ok' if r else 'misses'}": c for (m, r), c in sorted(agree.items())})
    sys.exit(1 if model_bad or rowan_bad else 0)


if __name__ == "__main__":
    main()
