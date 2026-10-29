"""Write bench/work/NAME/_all_modules.py: an import of every module of the benchmark's packages
(for `rowan hovers`: the module code of all of them once, then every function on demand).

Usage: python bench/hover_setup.py NAME  prints the file's path

The packages: PACKAGES' where listed; otherwise (functional) every top-level package and module
beside the driver. stdlib12k is one file: its path is printed, nothing written.
"""

import sys
from pathlib import Path

PACKAGES = {"pip": ["pip"], "timm": ["timm"], "sktime": ["sktime", "skbase"], "rich": ["rich"], "click": ["click"], "email": ["email"]}


def main() -> None:
    name = sys.argv[1]
    root = Path(__file__).resolve().parent / "work" / name
    if name == "stdlib12k":
        # (one file: its module code is all there is to import; nothing written beside it,
        # where a second file would change what `bench/work/stdlib12k/*.py` names)
        print(root / "big.py")
        return
    lines = []
    tops = PACKAGES.get(name) or sorted(p.name for p in root.iterdir() if p.is_dir() and (p / "__init__.py").exists())
    for pkg in tops:
        for p in sorted((root / pkg).rglob("*.py")):
            parts = p.relative_to(root).with_suffix("").parts
            if parts[-1] == "__init__":
                parts = parts[:-1]
            if parts[-1] == "__main__" or not all(x.isidentifier() for x in parts):
                continue
            lines.append("import " + ".".join(parts))
    if name not in PACKAGES:
        lines += [f"import {p.stem}" for p in sorted(root.glob("*.py")) if p.stem.isidentifier() and p.name not in ("main.py", "_all_modules.py")]
    out = root / "_all_modules.py"
    out.write_text("\n".join(lines) + "\n")
    print(out)


if __name__ == "__main__":
    main()
