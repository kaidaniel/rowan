"""Write bench/work/NAME/_all_modules.py: an import of every module of the benchmark's packages
(for `rowan hovers`: the module code of all of them once, then every function on demand).

Usage: python bench/hover_setup.py NAME  prints the file's path
"""

import sys
from pathlib import Path

PACKAGES = {"pip": ["pip"], "timm": ["timm"], "sktime": ["sktime", "skbase"], "rich": ["rich"], "click": ["click"], "email": ["email"]}


def main() -> None:
    name = sys.argv[1]
    root = Path(__file__).resolve().parent / "work" / name
    lines = []
    for pkg in PACKAGES[name]:
        for p in sorted((root / pkg).rglob("*.py")):
            parts = p.relative_to(root).with_suffix("").parts
            if parts[-1] == "__init__":
                parts = parts[:-1]
            if parts[-1] == "__main__" or not all(x.isidentifier() for x in parts):
                continue
            lines.append("import " + ".".join(parts))
    out = root / "_all_modules.py"
    out.write_text("\n".join(lines) + "\n")
    print(out)


if __name__ == "__main__":
    main()
