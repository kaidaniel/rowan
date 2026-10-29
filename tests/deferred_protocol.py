"""Deferred definitions (`interp/deferred.rs`: an imported module's `@d def f` whose decorators
are all plain inert code runs them when `f` is first read), checked exhaustively against CPython:

- a module `dlN` of the program's own (`LIB_HEAD`) with decorators (`DECOS`): plain ones that
  rowan defers (a `functools.wraps` wrapper, an attribute or `setattr` store, a call counter, a
  factory's closure, one reading a closure's cell another closure changes later, one from
  another module `dhN`, two stacked) and ones it must run at the `def` (a registry, a global
  counter, one that raises, one reading a name bound later or only on some paths, one giving
  back something else);
- where the `def` is (`PLACES`): module level, under a test, in a `try`, in a class body (a
  staticmethod), at module level beside an undecorated twin;
- what the module does after it (`AFTERS`): nothing, rebinds a name the decorator read (a
  global, a cell), reads `f` itself (an attribute store, a call, an alias), deletes or rebinds
  `f`, re-exports it from a second module;
- what the main module does with it (`USES`): calls it, reads its attributes (also after
  storing one, after rebinding the module's name the decorator read), its `__wrapped__`, through
  `vars`, `__dict__`, `getattr` by a name not known, `sys.modules`, `from … import`, a function
  of the program called later, a loop, a branch, unknown code given the module, `dir`,
  `callable`, `type`; or never reads it;
- the import at module level or in the entry function (`IMPORTS`);
- in a `try` naming the class of what it raises.

Observation points: the use's result (a scalar, else its `repr`) or the class it raised, the
module's registry (its effects in order) and its counter.

Each batch runs under every limit set (`LIMITS`): the default and `budget=0`.

    uv run --no-project python tests/deferred_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O1", "O2", "O3"]
LIMITS = (None, "budget=0")

HELPER = '''import functools

LABEL = "helper"


def ext_d(fn):
    @functools.wraps(fn)
    def inner(*a, **k):
        return fn(*a, **k) + 100

    inner.label = LABEL
    return inner
'''

LIB_HEAD = '''import functools
import os

import dh{N}

c1 = os.environ.get("C1") == "1"
c2 = os.environ.get("C2") == "1"
B = os.environ.get("B", "")
LABEL = "first"
ORDER = []
N = 0
if c1:
    MAYBE = "maybe"


def wraps_d(fn):
    @functools.wraps(fn)
    def inner(*a, **k):
        return fn(*a, **k) * 10

    inner.label = LABEL
    return inner


def attr_d(fn):
    fn.label = LABEL
    return fn


def setattr_d(fn):
    setattr(fn, "label", LABEL)
    return fn


def count_d(fn):
    @functools.wraps(fn)
    def inner(*a, **k):
        inner.calls += 1
        return fn(*a, **k)

    inner.calls = 0
    inner.label = LABEL
    return inner


def tag_with(x):
    def deco(fn):
        fn.label = x
        return fn

    return deco


def second_d(fn):
    fn.second = "s"
    return fn


def reg_d(fn):
    ORDER.append("reg:" + fn.__name__)
    return fn


def glob_d(fn):
    global N
    N += 1
    fn.label = N
    return fn


def raise_d(fn):
    if B == "K":
        raise KeyError("k")
    fn.label = B
    return fn


def late_d(fn):
    fn.label = LATE
    return fn


def maybe_d(fn):
    fn.label = MAYBE
    return fn


def other_d(fn):
    return len


def make_cell():
    label = "cell"

    def deco(fn):
        fn.label = label
        return fn

    def set_label(v):
        nonlocal label
        label = v

    return deco, set_label


cell_d, set_cell = make_cell()
'''

# the decorator lines (outermost first)
DECOS = {
    "wraps": ["@wraps_d"], "attr": ["@attr_d"], "setattr": ["@setattr_d"], "count": ["@count_d"],
    "factory": ["@tag_with('t')"], "ext": ["@dh{N}.ext_d"], "stacked": ["@wraps_d", "@second_d"],
    "reg": ["@reg_d"], "mixed": ["@attr_d", "@reg_d"], "glob": ["@glob_d"], "raise": ["@raise_d"], "late": ["@late_d"],
    "maybe": ["@maybe_d"], "other": ["@other_d"], "cell": ["@cell_d"],
}

BODY = ["def f(x):", "    ORDER.append('f')", "    return x + 1"]

PLACES = ["top", "if", "try", "class", "twin"]

AFTERS = {
    "none": [],
    "relabel": ["LABEL = 'second'"],
    "touch": ["f.extra = 1", "LABEL = 'second'"],
    "early_call": ["EARLY = f(0)"],
    "alias": ["ALIAS = f", "LABEL = 'second'"],
    "del": ["del f"],
    "rebind": ["f = None"],
    "reexport": [],
    "setcell": ["set_cell('changed')"],
}

USES = {
    "call": ["r = M.f(2)"],
    "label": ["r = M.f.label"],
    "name": ["r = M.f.__name__"],
    "twice": ["r = f'{M.f(1)}/{M.f(2)}/{getattr(M.f, \"calls\", -1)}'"],
    "wrapped": ["r = hasattr(M.f, '__wrapped__')"],
    "vars": ["r = vars(M)['f'](3)"],
    "dict": ["r = M.__dict__['f'](3)"],
    "getattr": ["r = getattr(M, os.environ.get('FN{N}', 'f'))(3)"],
    "sysmod": ["r = sys.modules['dl{N}'].f.label"],
    "is": ["r = M.f is M.f"],
    "branch": ["x = 'none'", "if c1:", "    x = M.f.label", "r = f'{x}/{M.f.label}'"],
    "after_set": ["M.LABEL = 'main'", "r = M.f.label"],
    "store": ["M.f.label = 'z'", "r = M.f.label"],
    "helper": ["r = h{N}()"],
    "loop": ["for i in range(2):", "    r = M.f(i)"],
    "unknown": ["timeit.timeit('m.f(0)', globals={'m': M}, number=1)", "r = len(M.ORDER)"],
    "dir": ["r = 'f' in dir(M)"],
    "callable": ["r = callable(M.f)"],
    "type": ["r = type(M.f).__name__"],
    "never": ["r = M.N"],
    "reexport": ["r = M2.f.label"],
    "from": ["r = F{N}.label"],
    "alias": ["r = getattr(M, 'ALIAS', None) is M.f"],
}

IMPORTS = ["module", "local"]

TAIL = [
    "except TypeError:",
    "    r = 'TypeError!'",
    "except KeyError:",
    "    r = 'KeyError!'",
    "except NameError:",
    "    r = 'NameError!'",
    "except AttributeError:",
    "    r = 'AttributeError!'",
    "except Exception:",
    "    r = 'Exception!'",
]


def lib_text(deco: str, place: str, after: str) -> str:
    decos = [d.replace("{N}", "{N}") for d in DECOS[deco]]
    if place == "class":
        defn = ["class K:", "    @staticmethod", *("    " + d for d in decos), *("    " + ln for ln in BODY), "", "", "f = K.f"]
    else:
        defn = [*decos, *BODY]
        if place == "if":
            defn = ["if c2:", *("    " + ln for ln in defn), "else:", "    def f(x):", "        return -1"]
        elif place == "try":
            defn = ["try:", *("    " + ln for ln in defn), "except Exception:", "    f = None"]
        elif place == "twin":
            defn = [*defn, "", "", "def twin(x):", "    return x"]
    lines = [LIB_HEAD, *defn, "", "", *AFTERS[after], "LATE = 'late'", ""]
    return "\n".join(lines)


def program(deco: str, place: str, after: str, use: str, imp: str) -> Prog:
    ind = lambda ls, k=1: ["    " * k + ln for ln in ls]
    m = "m" if imp == "local" else "m{N}"
    steps = [ln.replace("M2", "m2{N}" if imp == "module" else "m2").replace("M", m) for ln in USES[use]]
    lines = ["import sys", "import timeit"]
    # (a module's code may raise: the decorator `raise_d`, `from … import f` after `del f`)
    if imp == "module":
        lines += ["try:", "    import dl{N} as m{N}", "except Exception:", "    m{N} = None"]
        if use == "reexport":
            lines += ["try:", "    import dr{N} as m2{N}", "except Exception:", "    m2{N} = None"]
    if use == "from":
        lines += ["try:", "    from dl{N} import f as F{N}", "except Exception:", "    F{N} = None"]
    if use == "helper":
        lines += ["def h{N}():", "    import dl{N}", "    return dl{N}.f(5)"]
    lines += ["def p{N}():", "    global O1, O2, O3"]
    if imp == "local":
        steps = ["import dl{N} as m", *(["import dr{N} as m2"] if use == "reexport" else []), *steps]
    lines += ["    r = None", *(["    m = None"] if imp == "local" else []), "    try:", *ind(steps, 2), *ind(TAIL)]
    lines += ["    if r is None or isinstance(r, (bool, int, float, str)):", "        O1 = r", "    else:", "        O1 = repr(r)"]
    # (the module's state where its import completed: an observation line must not raise)
    mm = "m" if imp == "local" else "m{N}"
    lines += ["    o2, o3 = '-', -1", f"    if {mm} is not None:", f"        o2 = '|'.join({mm}.ORDER)", f"        o3 = {mm}.N",
              "    O2 = o2", "    O3 = o3"]
    fs = {"dl{N}": lib_text(deco, place, after), "dh{N}": HELPER}
    if use == "reexport":
        fs["dr{N}"] = "from dl{N} import f\n\nX = 1\n"
    return Prog(lines, desc=f"{deco} {place} {after} {use} {imp}", files=fs)


def programs():
    for deco, place, after, use, imp in itertools.product(DECOS, PLACES, AFTERS, USES, IMPORTS):
        # (the after-step `reexport` goes with the use that reads through it, and only it)
        if (after == "reexport") != (use == "reexport"):
            continue
        yield program(deco, place, after, use, imp)


if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=LIMITS)
