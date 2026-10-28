"""The argparse model, checked exhaustively against CPython (AGENTS.md, Models;
`models/argparse.rs`: `parse_args` on a known argv parsed the way argparse parses it, the
Namespace's attributes and values; on an unknown argv every attribute a parse can set), in the
style of `lib_protocol.py`:

- an argv (`INPUTS`): none, options with separate and attached values, flags, counted and
  repeated options, an abbreviation, `--`, a bad choice, an unknown option, a value of the
  wrong type, two argvs by a branch, and an argv not known (an environment variable split);
- a parser (`PARSERS`: options of each action and type, a required option, subcommands,
  optional positionals) and one attribute of what it parses (`ATTRS`), or `exit` for a
  `SystemExit`;
- in a `try` naming the class of what it raises.

Observation point: the attribute's value (a scalar, else its `repr`), `exit`, or the class
raised.

    uv run --no-project python tests/argparse_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import program
from protocol_lib import cli

NAMES = ["O"]

PARSE = ["    try:", "        with contextlib.redirect_stderr(io.StringIO()):", "            return ap.parse_args(argv)",
         "    except SystemExit:", "        return 'exit'"]

PARSERS = {
    "main": ["def MAIN(argv):", "    ap = argparse.ArgumentParser(prog='p')", "    ap.add_argument('--n', type=int, default=1)",
             "    ap.add_argument('-v', '--verbose', action='store_true')", "    ap.add_argument('--tag', action='append')",
             "    ap.add_argument('-c', action='count', default=0)", "    ap.add_argument('--mode', choices=['a', 'b'], default='a')",
             "    ap.add_argument('files', nargs='*')", *PARSE],
    "required": ["def REQ(argv):", "    ap = argparse.ArgumentParser(prog='p')", "    ap.add_argument('--n', type=float, required=True)",
                 "    ap.add_argument('-v', action='store_const', const=7, dest='level')", "    ap.add_argument('pos', nargs='?', default='d')", *PARSE],
    "sub": ["def SUB(argv):", "    ap = argparse.ArgumentParser(prog='p')", "    ap.add_argument('-v', action='store_true')",
            "    sp = ap.add_subparsers(dest='cmd')", "    a = sp.add_parser('run')", "    a.add_argument('--n', type=int, default=5)",
            "    b = sp.add_parser('list')", "    b.add_argument('files', nargs='+')", *PARSE],
}

ATTRS = ["n", "verbose", "tag", "c", "mode", "files", "level", "pos", "cmd", "v"]

LIB = {
    "imports": ["import argparse", "import io"],
    "inputs": {
        "none": "[]", "n": "['--n', '3']", "attached": "['--n=4', 'x', 'y']", "flags": "['-v', '-cc', 'f']",
        "append": "['--tag', 'a', '--tag', 'b']", "choice": "['--mode', 'z']", "bogus": "['--bogus']", "wrong_type": "['--n', 'x']",
        "abbrev": "['--verb']", "dashdash": "['--', '-v']", "run": "['run', '--n', '2']", "list": "['-v', 'list', 'a', 'b']",
        "joined": "['--n', '1'] if c1 else ['-v']", "unknown": "os.environ.get('RA{N}', '--n 2').split()",
    },
    "ops": {},
    "helpers": [ln for lines in PARSERS.values() for ln in lines],
}

for p in PARSERS:
    fn = PARSERS[p][0].split()[1].split("(")[0]
    for a in ATTRS:
        LIB["ops"][f"{p}_{a}"] = f"(lambda r: r if r == 'exit' else getattr(r, {a!r}, 'none'))({fn}(v))"
    LIB["ops"][f"{p}_vars"] = f"(lambda r: r if r == 'exit' else sorted(vars(r)))({fn}(v))"


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        yield program(LIB, iv, ov, f"argparse {ik} {ok}")


if __name__ == "__main__":
    cli(programs, NAMES)
