"""The pure-path models, checked exhaustively against CPython (AGENTS.md, Models;
`models/pathtext.rs`: `os.path` and pure `pathlib` paths on known text, POSIX), in the style of
`lib_protocol.py`:

- a path text (`INPUTS`): absolute and relative, a trailing slash, `.`, empty, the root, `..`
  parts, several suffixes, a hidden file, doubled slashes, two texts by a branch, and text
  not known (an environment variable);
- one operation on it (`OPS`): `PurePosixPath` parts (`name`, `suffix`, `suffixes`, `stem`,
  `parent`, `parents`, `parts`, `anchor`), joins, `with_suffix` / `with_name` / `with_stem`
  (with bad arguments too), `relative_to`, `is_absolute`, `match`, `as_posix`, comparisons,
  and `os.path` (`basename`, `dirname`, `splitext`, `split`, `join`, `normpath`, `isabs`,
  `commonprefix`);
- in a `try` naming the class of what it raises.

Observation point: the result (a scalar, else its `repr`) or the class it raised.

    uv run --no-project python tests/pathlib_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import program
from protocol_lib import cli

NAMES = ["O"]

LIB = {
    "imports": ["import os.path", "from pathlib import PurePosixPath as P"],
    "inputs": {
        "abs": "'/a/b/c.txt'", "rel": "'a/b/'", "dot": "'.'", "empty": "''", "root": "'/'", "up": "'a/../b.tar.gz'",
        "hidden": "'/x/.env'", "double": "'//a//b'", "joined": "'/a/b.py' if c1 else 'c'",
        "unknown": "os.environ.get('RP{N}', 'u/v.w')",
    },
    "ops": {
        "name": "P(v).name", "suffix": "P(v).suffix", "suffixes": "P(v).suffixes", "stem": "P(v).stem", "parent": "str(P(v).parent)",
        "parent2": "str(P(v).parent.parent)", "parents_len": "len(P(v).parents)", "parents0": "str(P(v).parents[0])", "parts": "P(v).parts",
        "anchor": "P(v).anchor", "str": "str(P(v))", "join": "str(P(v) / 'x')", "join_abs": "str(P(v) / '/y')", "joinpath": "str(P(v).joinpath('q', 'r'))",
        "with_suffix": "str(P(v).with_suffix('.py'))", "with_suffix_bad": "str(P(v).with_suffix('py'))", "with_suffix_empty": "str(P(v).with_suffix(''))",
        "with_name": "str(P(v).with_name('n.txt'))", "with_stem": "str(P(v).with_stem('s'))", "relative": "str(P(v).relative_to('/a'))",
        "is_absolute": "P(v).is_absolute()", "match": "P(v).match('*.txt')", "as_posix": "P(v).as_posix()", "eq": "P(v) == P('a/b')",
        "lt": "P(v) < P('b')", "is_relative": "P(v).is_relative_to('/a')", "rtruediv": "str('base' / P(v))",
        "basename": "os.path.basename(v)", "dirname": "os.path.dirname(v)", "splitext": "os.path.splitext(v)", "split": "os.path.split(v)",
        "path_join": "os.path.join(v, 'x', '')", "path_join_abs": "os.path.join('a', v)", "normpath": "os.path.normpath(v)",
        "isabs": "os.path.isabs(v)", "commonprefix": "os.path.commonprefix([v, '/a/x'])", "splitext_name": "os.path.splitext(os.path.basename(v))[0]",
    },
}


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        yield program(LIB, iv, ov, f"pathlib {ik} {ok}")


if __name__ == "__main__":
    cli(programs, NAMES)
