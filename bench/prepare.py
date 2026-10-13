"""Build one benchmark program under bench/work/<name>/ (run through `uv run --no-project`).

The programs are large, real Python code, pinned so that every run analyses the same text:

  stdlib12k  _pydecimal.py + argparse.py + pydoc.py of the pinned CPython (.python-version),
             concatenated into one 12k-line file (no driver: rowan's entry runs do the work)
  email      the stdlib `email` package with a small driver (drivers/email_main.py)
  rich       rich==RICH_VERSION from PyPI (wheel checked against PyPI's sha256) with a driver
  click      click==CLICK_VERSION from PyPI likewise, with a driver (commands built by
             decorators, run through its test runner: dense, dynamic object graphs)

Usage: python bench/prepare.py NAME     prints the path of the file to analyse
"""

import hashlib
import json
import shutil
import sys
import sysconfig
import urllib.request
import zipfile
from pathlib import Path

RICH_VERSION = "13.9.4"
CLICK_VERSION = "8.1.7"
HERE = Path(__file__).resolve().parent
WORK = HERE / "work"
STDLIB = Path(sysconfig.get_paths()["stdlib"])


def stdlib12k() -> Path:
    d = WORK / "stdlib12k"
    d.mkdir(parents=True, exist_ok=True)
    out = d / "big.py"
    out.write_text("".join((STDLIB / f).read_text() for f in ["_pydecimal.py", "argparse.py", "pydoc.py"]))
    return out


def email() -> Path:
    d = WORK / "email"
    shutil.rmtree(d, ignore_errors=True)
    shutil.copytree(STDLIB / "email", d / "email", ignore=shutil.ignore_patterns("__pycache__"))
    shutil.copy(HERE / "drivers" / "email_main.py", d / "main.py")
    return d / "main.py"


def pypi_package(name: str, version: str, driver: str) -> Path:
    """A pure-Python package from its PyPI wheel (sha256 checked), with a driver as main.py."""
    d = WORK / name
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    meta = json.load(urllib.request.urlopen(f"https://pypi.org/pypi/{name}/{version}/json"))
    wheel = next(u for u in meta["urls"] if u["packagetype"] == "bdist_wheel")
    data = urllib.request.urlopen(wheel["url"]).read()
    if hashlib.sha256(data).hexdigest() != wheel["digests"]["sha256"]:
        sys.exit(f"{name} {version}: sha256 mismatch")
    whl = d / wheel["filename"]
    whl.write_bytes(data)
    with zipfile.ZipFile(whl) as z:
        for member in z.namelist():
            if member.startswith(f"{name}/"):
                z.extract(member, d)
    whl.unlink()
    shutil.copy(HERE / "drivers" / driver, d / "main.py")
    return d / "main.py"


def rich() -> Path:
    return pypi_package("rich", RICH_VERSION, "rich_main.py")


def click() -> Path:
    return pypi_package("click", CLICK_VERSION, "click_main.py")


PROGRAMS = {"stdlib12k": stdlib12k, "email": email, "rich": rich, "click": click}

if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in PROGRAMS:
        sys.exit(f"usage: prepare.py {{{'|'.join(PROGRAMS)}}}")
    path = PROGRAMS[sys.argv[1]]()
    lines = sum(p.read_text().count("\n") for p in path.parent.rglob("*.py"))
    print(f"{sys.argv[1]}: {lines} lines", file=sys.stderr)
    print(path)
