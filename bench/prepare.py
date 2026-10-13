"""Build one benchmark program under bench/work/<name>/ (run through `uv run --no-project`).

The programs are large, real Python code, pinned so that every run analyses the same text:

  stdlib12k  _pydecimal.py + argparse.py + pydoc.py of the pinned CPython (.python-version),
             concatenated into one 12k-line file (no driver: rowan's entry runs do the work)
  email      the stdlib `email` package with a small driver (drivers/email_main.py)
  rich       rich==RICH_VERSION from PyPI (wheel checked against PyPI's sha256) with a driver
  click      click==CLICK_VERSION from PyPI likewise, with a driver (commands built by
             decorators, run through its test runner: dense, dynamic object graphs)

The large frontier (each ~100k-400k lines over hundreds of files; wheels from PyPI, sha256
checked, test packages and compiled extensions left out; a driver as main.py):

  pip        pip (stdlib only: its dependencies are vendored), driven through its commands
  timm       PyTorch Image Models: torch models, layers, optimizers, schedulers, data transforms
  sktime     sktime + scikit-base: time series on pandas (forecasters, transformers, mtypes)
  functional functional-style libraries (toolz, funcy, returns, expression, pyrsistent,
             reactivex, parsy, lark, pyparsing, more-itertools, pydash, glom, hypothesis, …)
             used together by one functional pipeline

Usage: python bench/prepare.py NAME     prints the path of the file to analyse
"""

import hashlib
import io
import json
import shutil
import sys
import sysconfig
import urllib.request
import zipfile
from pathlib import Path

RICH_VERSION = "13.9.4"
CLICK_VERSION = "8.1.7"
PIP = [("pip", "26.2.1")]
TIMM = [("timm", "1.0.30")]
SKTIME = [("sktime", "1.2.0"), ("scikit-base", "1.2.0")]
FUNCTIONAL = [
    ("toolz", "1.2.0"), ("funcy", "2.1"), ("returns", "0.29.0"), ("expression", "5.7.0"),
    ("pyrsistent", "0.20.0"), ("reactivex", "5.1.0"), ("parsy", "2.2"), ("lark", "1.3.1"),
    ("pyparsing", "3.3.3"), ("more-itertools", "11.2.0"), ("pydash", "8.1.0"), ("glom", "25.12.0"),
    ("pyfunctional", "1.5.0"), ("funcparserlib", "1.0.1"), ("hypothesis", "6.155.7"),
    ("attrs", "26.1.0"), ("sortedcontainers", "2.4.0"),
    # their dependencies
    ("typing-extensions", "4.16.0"), ("tabulate", "0.10.0"), ("dill", "0.4.1"), ("boltons", "26.2.0"),
    ("face", "26.0.1"),
]
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


def wheel_sources(dest: Path, name: str, version: str) -> int:
    """One PyPI wheel's package files (sha256 checked) under dest, without test packages and
    compiled extensions; the py3-none-any wheel where there is one, else the first by file
    name (the files kept are alike in every platform's wheel). Returns its lines of Python."""
    meta = json.load(urllib.request.urlopen(f"https://pypi.org/pypi/{name}/{version}/json"))
    wheels = sorted((u for u in meta["urls"] if u["packagetype"] == "bdist_wheel"), key=lambda u: ("none-any" not in u["filename"], u["filename"]))
    wheel = wheels[0]
    data = urllib.request.urlopen(wheel["url"]).read()
    if hashlib.sha256(data).hexdigest() != wheel["digests"]["sha256"]:
        sys.exit(f"{name} {version}: sha256 mismatch")
    lines = 0
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for member in z.namelist():
            parts = member.split("/")
            if member.endswith("/") or member.endswith((".so", ".pyd", ".dll")) or ".dist-info" in parts[0] or ".data" in parts[0]:
                continue
            if any(p in ("tests", "test") for p in parts[:-1]) or parts[-1] == "conftest.py":
                continue
            z.extract(member, dest)
            if member.endswith(".py"):
                lines += z.read(member).count(b"\n")
    return lines


def wheel_program(prog: str, dists: list[tuple[str, str]], driver: str) -> Path:
    """Several wheels' sources side by side under bench/work/PROG, with a driver as main.py."""
    d = WORK / prog
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    for name, version in dists:
        n = wheel_sources(d, name, version)
        print(f"  {name}=={version}: {n} lines", file=sys.stderr)
    shutil.copy(HERE / "drivers" / driver, d / "main.py")
    return d / "main.py"


def pip() -> Path:
    return wheel_program("pip", PIP, "pip_main.py")


def timm() -> Path:
    return wheel_program("timm", TIMM, "timm_main.py")


def sktime() -> Path:
    return wheel_program("sktime", SKTIME, "sktime_main.py")


def functional() -> Path:
    return wheel_program("functional", FUNCTIONAL, "functional_main.py")


def rich() -> Path:
    return pypi_package("rich", RICH_VERSION, "rich_main.py")


def click() -> Path:
    return pypi_package("click", CLICK_VERSION, "click_main.py")


PROGRAMS = {"stdlib12k": stdlib12k, "email": email, "rich": rich, "click": click,
            "pip": pip, "timm": timm, "sktime": sktime, "functional": functional}

if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in PROGRAMS:
        sys.exit(f"usage: prepare.py {{{'|'.join(PROGRAMS)}}}")
    path = PROGRAMS[sys.argv[1]]()
    lines = sum(p.read_text().count("\n") for p in path.parent.rglob("*.py"))
    files = sum(1 for _ in path.parent.rglob("*.py"))
    print(f"{sys.argv[1]}: {lines} lines in {files} files", file=sys.stderr)
    print(path)
