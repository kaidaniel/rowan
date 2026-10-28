"""The configparser model, checked exhaustively against CPython (AGENTS.md, Models;
`models/configparser.rs`: a parser over literal INI text simulated, its sections and options
closed dicts of interpolated strings), in the style of `lib_protocol.py`:

- the INI text (`INPUTS`): sections with options, DEFAULT inheritance, `%(x)s` interpolation,
  mixed case, empty, text without a section header, a duplicate section, two texts by a
  branch, and text not known (an environment variable);
- one read of the parsed text (`OPS`): `sections`, `get` / `getint` / `getboolean` /
  `getfloat` (with `fallback` too), `has_section` / `has_option`, `items`, `options`,
  `defaults`, subscripts of the parser and of a section, `in`, raw `get`, and the errors
  (a missing section or option, a value that is no int, bad interpolation);
- in a `try` naming the class of what it raises (configparser's errors: `Exception!`).

Observation point: the result (a scalar, else its `repr`) or the class it raised.

    uv run --no-project python tests/config_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import program
from protocol_lib import cli

NAMES = ["O"]

LIB = {
    "imports": ["import configparser"],
    "inputs": {
        "plain": "'[s]\\nk = 1\\nname = x\\n'",
        "defaults": "'[DEFAULT]\\nk = 7\\nflag = yes\\n[s]\\nname = y\\n[t]\\nk = 2\\n'",
        "interp": "'[s]\\nbase = /a\\npath = %(base)s/b\\nk = 3\\n'",
        "bad_interp": "'[s]\\npath = %(nope)s/b\\nk = 4\\n'",
        "case": "'[S]\\nKey = V\\n[s]\\nk = 5\\n'",
        "empty": "''",
        "no_header": "'k = 1\\n'",
        "dup": "'[s]\\nk = 1\\n[s]\\nk = 2\\n'",
        "notint": "'[s]\\nk = one\\nflag = maybe\\n'",
        "joined": "'[s]\\nk = 1\\n' if c1 else '[t]\\nk = 2\\n'",
        "unknown": "os.environ.get('RG{N}', '[s]\\nk = 9\\n')",
    },
    "ops": {
        "sections": "CP(v).sections()", "get": "CP(v).get('s', 'k')", "getint": "CP(v).getint('s', 'k')",
        "getfloat": "CP(v).getfloat('s', 'k')", "getbool": "CP(v).getboolean('s', 'flag')", "fallback": "CP(v).get('s', 'zz', fallback='d')",
        "getint_fallback": "CP(v).getint('t', 'k', fallback=-1)", "has_section": "CP(v).has_section('t')", "has_option": "CP(v).has_option('s', 'k')",
        "items": "sorted(CP(v).items('s'))", "options": "sorted(CP(v).options('s'))", "defaults": "dict(CP(v).defaults())",
        "sub": "CP(v)['s']['k']", "sub_missing": "CP(v)['nope']", "opt_missing": "CP(v)['s']['zz']", "in": "'s' in CP(v)",
        "in_section": "'k' in CP(v)['s']", "raw": "CP(v).get('s', 'path', raw=True)", "path": "CP(v).get('s', 'path')",
        "name": "CP(v).get('s', 'name')", "upper": "CP(v)['s']['K']", "section_get": "CP(v)['s'].get('k', 'd')",
        "default_key": "CP(v)['t'].get('flag')", "len": "len(CP(v))", "keys": "list(CP(v))",
    },
    "helpers": ["def CP(v):", "    c = configparser.ConfigParser()", "    c.read_string(v)", "    return c"],
}


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        yield program(LIB, iv, ov, f"configparser {ik} {ok}")


if __name__ == "__main__":
    cli(programs, NAMES)
