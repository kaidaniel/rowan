"""Imports every bench/*.py script and checks every import they make, also inside functions (ci.yml).

    uv run --no-project python bench/check_imports.py

A bench script must do no work at import: this imports each one (a script that runs work, or
imports a module that is gone, fails here), then resolves each `import M` / `from M import n`
anywhere in its text, with bench/ and tests/ on the path as the scripts put them: a module of
bench/ or tests/ must exist and define the names taken from it; another module must be found.
Exits 1 listing what failed. Takes well under a second.
"""
from __future__ import annotations

import ast
import importlib
import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
LOCAL = [HERE, ROOT / "tests"]


def problems() -> list[str]:
    sys.dont_write_bytecode = True
    for d in reversed(LOCAL):
        sys.path.insert(0, str(d))
    local = {p.stem for d in LOCAL for p in d.glob("*.py")}
    out: list[str] = []
    for path in sorted(HERE.glob("*.py")):
        try:
            importlib.import_module(path.stem)
        except BaseException as e:  # noqa: BLE001 - a SystemExit at import is a failure too
            out.append(f"{path.name}: importing it raised {type(e).__name__}: {e}")
            continue
        for node in ast.walk(ast.parse(path.read_text(), str(path))):
            if isinstance(node, ast.Import):
                wanted = [(a.name, []) for a in node.names]
            elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                wanted = [(node.module, [a.name for a in node.names if a.name != "*"])]
            else:
                continue
            for mod, names in wanted:
                top = mod.split(".")[0]
                if top not in local:
                    if importlib.util.find_spec(top) is None:
                        out.append(f"{path.name}:{node.lineno}: no module {mod}")
                    continue
                try:
                    m = importlib.import_module(mod)
                except BaseException as e:  # noqa: BLE001
                    out.append(f"{path.name}:{node.lineno}: importing {mod} raised {type(e).__name__}: {e}")
                    continue
                out += [f"{path.name}:{node.lineno}: {mod} has no {n}" for n in names if not hasattr(m, n)]
    return out


if __name__ == "__main__":
    bad = problems()
    print("\n".join(bad) or f"bench: {len(list(HERE.glob('*.py')))} scripts import cleanly")
    sys.exit(1 if bad else 0)
