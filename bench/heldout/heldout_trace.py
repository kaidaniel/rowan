"""A pytest plugin (`-p heldout_trace`) recording what a held-out package's own test suite does at
the places rowan makes claims (bench/heldout.py `sound`). It runs inside the package's venv and
imports nothing of rowan's.

HELDOUT_POINTS names a JSON file: per source file, its statements to watch (`[start, end, names]`)
and the lines of its key / attribute reads with a verdict. HELDOUT_OUT is where the observations
go, at the end of the session:

* per statement (`file:end`), after each run of it that completed (the frame moved on to a line
  outside it, or returned; a statement that raised is not observed): each name's value,
  summarised (`summary`), distinct summaries up to MAX_DISTINCT, and how many runs;
* per verdict line: how many times it ran, and the KeyErrors / AttributeErrors raised there
  (raised in that frame, not passing through it from a callee), by key.
"""
from __future__ import annotations

import enum
import json
import os
import sys
import threading

MAX_DISTINCT = 40
SCALARS = (bool, int, float, str, bytes, type(None))


def summary(v, roots):
    """(kind, data, type name): `lit` a scalar (its repr; None when longer than 300 characters),
    `tuple` of up to 8 scalars (its repr), `dict` (its keys' reprs, None past 64), `inst` an
    instance of a class from the package (`roots`: its source directories; not a builtin's subclass
    or an Enum member) with its `__dict__`'s keys, `obj` anything else."""
    t = type(v)
    if t in SCALARS:
        r = repr(v)
        return ("lit", r if len(r) <= 300 else None, t.__name__)
    if t is tuple and len(v) <= 8 and all(type(x) in SCALARS for x in v):
        r = repr(v)
        return ("tuple", r if len(r) <= 300 else None, "tuple")
    if t is dict:
        return ("dict", sorted(repr(k) for k in v) if len(v) <= 64 else None, "dict")
    # (builtin subclasses keep keys or attributes rowan models elsewhere; Enum members: their
    # class's; neither compared by keys)
    if isinstance(v, (dict, list, set, tuple, str, bytes, int, float)) or isinstance(v, enum.Enum):
        return ("obj", None, getattr(t, "__qualname__", t.__name__))
    mod = sys.modules.get(getattr(t, "__module__", ""), None)
    f = getattr(mod, "__file__", None) or ""
    d = getattr(v, "__dict__", None)
    if isinstance(d, dict) and not isinstance(v, type) and any(f.startswith(r) for r in roots):
        return ("inst", sorted(repr(k) for k in d) if len(d) <= 64 else None, t.__qualname__)
    return ("obj", None, getattr(t, "__qualname__", t.__name__))


class Recorder:
    def __init__(self, points: dict, roots: list):
        self.roots = roots
        self.stmt_at = {}  # file -> line -> (start, end, names)
        self.verdict_lines = {}
        for f, spec in points.items():
            m = self.stmt_at.setdefault(f, {})
            for start, end, names in spec.get("stmts", []):
                for ln in range(start, end + 1):
                    m[ln] = (start, end, tuple(names))
            self.verdict_lines[f] = set(spec.get("verdict_lines", []))
        self.files = set(self.stmt_at) | set(self.verdict_lines)
        self.values = {}  # "file:end" -> {"runs": n, "names": {name: [summaries]}}
        self.lines = {}  # "file:line" -> {"runs": n, "raised": {key repr: n}}
        self.pending = {}  # frame id -> (file, stmt)
        self.lock = threading.RLock()

    def observe(self, frame, f, stmt):
        start, end, names = stmt
        rec = self.values.setdefault(f"{f}:{end}", {"runs": 0, "names": {}})
        rec["runs"] += 1
        loc = frame.f_locals
        for n in names:
            if n not in loc:
                continue
            try:
                s = summary(loc[n], self.roots)
            except Exception:  # noqa: BLE001  (a value whose repr fails)
                continue
            seen = rec["names"].setdefault(n, [])
            if len(seen) < MAX_DISTINCT and list(s) not in seen:
                seen.append(list(s))

    def local(self, frame, event, arg):
        with self.lock:
            f = frame.f_code.co_filename
            fid = id(frame)
            if event == "line":
                ln = frame.f_lineno
                p = self.pending.get(fid)
                if p is not None and not (p[1][0] <= ln <= p[1][1]):
                    del self.pending[fid]
                    self.observe(frame, f, p[1])
                    p = None
                st = self.stmt_at.get(f, {}).get(ln)
                if st is not None and p is None:
                    self.pending[fid] = (f, st)
                if ln in self.verdict_lines.get(f, ()):
                    r = self.lines.setdefault(f"{f}:{ln}", {"runs": 0, "raised": {}})
                    r["runs"] += 1
            elif event == "return":
                p = self.pending.pop(fid, None)
                if p is not None:
                    self.observe(frame, f, p[1])
            elif event == "exception":
                # (the statement running in this frame did not complete)
                self.pending.pop(fid, None)
                e, tb = arg[1], arg[2]
                ln = frame.f_lineno
                if ln in self.verdict_lines.get(f, ()) and tb is not None and tb.tb_next is None:
                    key = None
                    if isinstance(e, KeyError) and e.args:
                        key = e.args[0]
                    elif isinstance(e, AttributeError):
                        key = getattr(e, "name", None)
                    if key is not None:
                        r = self.lines.setdefault(f"{f}:{ln}", {"runs": 0, "raised": {}})
                        try:
                            k = repr(key)
                        except Exception:  # noqa: BLE001
                            k = "?"
                        r["raised"][k] = r["raised"].get(k, 0) + 1
        return self.local

    def glob(self, frame, event, arg):
        if event == "call" and frame.f_code.co_filename in self.files:
            return self.local
        return None

    def dump(self, path):
        with open(path, "w") as fh:
            json.dump({"values": self.values, "lines": self.lines}, fh)


REC = None


def pytest_configure(config):
    global REC
    with open(os.environ["HELDOUT_POINTS"]) as fh:
        spec = json.load(fh)
    REC = Recorder(spec["files"], spec["roots"])
    threading.settrace(REC.glob)
    sys.settrace(REC.glob)


def pytest_unconfigure(config):
    sys.settrace(None)
    threading.settrace(None)  # type: ignore[arg-type]
    if REC is not None:
        REC.dump(os.environ["HELDOUT_OUT"])
