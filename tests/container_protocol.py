"""The container models, checked exhaustively against CPython (AGENTS.md, Models; `containers.rs`,
`builtins.rs`: list, dict, set and tuple methods and the builtins over them):

- a receiver `c` (`RECEIVERS`): list, dict, set and tuple literals (empty, of ints, mixed,
  unsorted, nested), two of them joined by a branch, and containers of unknown items
  (`json.loads(...)` of an environment variable: its run's value fixed);
- one operation on it (`OPS`): every method of its type with literal arguments (also of the
  wrong type or a missing key / item), operators, subscripts and slices, `del`, unpacking, and
  the builtins over containers (`sorted`, `min`, `sum`, `dict(zip(...))`, `enumerate`, …);
- in a `try` naming the class of what it raises.

Observation points: the result (a scalar, else its `repr`) or the class it raised, then the
receiver's `repr` after the operation.

`--pairs`: one of `FIRST` (an operation that changes the receiver) then any operation of its
type on it, each in its own `try`, both observed.

    uv run --no-project python tests/container_protocol.py [--pairs] [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O"]

RECEIVERS = {
    "list": {
        "empty": "[]", "ints": "[3, 1, 2]", "mixed": "[1, 'a', None]", "nested": "[[1], [2, 3]]", "dups": "[1, 1, 2]",
        "joined": "[1, 2] if c1 else ['x']", "unknown": "json.loads(os.environ.get('RL{N}', '[2, 1]'))",
    },
    "dict": {
        "empty": "{}", "two": "{'a': 1, 'b': 2}", "intkeys": "{1: 'x', 2: 'y'}", "nested": "{'a': {'b': 1}}",
        "joined": "{'a': 1} if c1 else {'b': 2, 'a': 3}", "unknown": "json.loads(os.environ.get('RD{N}', '{\"a\": 5}'))",
    },
    "set": {
        "empty": "set()", "ints": "{1, 2}", "strs": "{'a'}", "joined": "{1} if c1 else {2, 3}",
        "unknown": "set(json.loads(os.environ.get('RS{N}', '[4]')))",
    },
    "tuple": {
        "empty": "()", "ints": "(2, 1)", "mixed": "(1, 'a')", "joined": "(1,) if c1 else (1, 2)",
        "unknown": "tuple(json.loads(os.environ.get('RT{N}', '[7, 8]')))",
    },
}

OPS = {
    "list": {
        "append": "c.append(4)", "extend": "c.extend([5, 6])", "extend_str": "c.extend('ab')", "insert": "c.insert(0, 9)",
        "insert_far": "c.insert(10, 9)", "pop": "c.pop()", "pop0": "c.pop(0)", "pop_far": "c.pop(5)", "remove": "c.remove(1)",
        "index": "c.index(2)", "count": "c.count(1)", "sort": "c.sort()", "sort_rev": "c.sort(reverse=True)", "reverse": "c.reverse()",
        "copy": "c.copy()", "clear": "c.clear()", "add": "c + [0]", "add_bad": "c + (0,)", "mul": "c * 2", "iadd": "c += [7]\nlen(c)",
        "item": "c[0]", "item_neg": "c[-1]", "slice": "c[1:]", "slice_step": "c[::-1]", "set_item": "c[0] = 'z'\nc[0]",
        "set_slice": "c[0:1] = [8, 9]\nlen(c)", "del_item": "del c[0]\nlen(c)", "in": "2 in c", "len": "len(c)",
        "sorted": "sorted(c)", "min": "min(c)", "max": "max(c)", "sum": "sum(c)", "any": "any(c)", "all": "all(c)",
        "enumerate": "list(enumerate(c))", "zip": "list(zip(c, 'xyz'))", "reversed": "list(reversed(c))", "set": "set(c) == set(c)",
        "dict_zip": "dict(zip(c, range(3)))", "unpack": "(lambda a, *b: b)(*c)", "unpack2": "(lambda a, b: a)(*c)", "eq": "c == [3, 1, 2]",
        "lt": "c < [2]", "join": "','.join(c)", "str": "str(c)", "bool": "bool(c)", "tuple": "tuple(c)", "star": "[*c, *c]",
    },
    "dict": {
        "get": "c.get('a')", "get_d": "c.get('z', 0)", "item": "c['a']", "item_int": "c[1]", "pop": "c.pop('a')", "pop_d": "c.pop('z', None)",
        "pop_missing": "c.pop('z')", "popitem": "c.popitem()", "setdefault": "c.setdefault('a', 7)", "setdefault_new": "c.setdefault('n', [])",
        "update": "c.update({'b': 9, 'c': 1})", "update_kw": "c.update(z=1)", "update_pairs": "c.update([('q', 2)])", "update_bad": "c.update(5)",
        "keys": "list(c.keys())", "values": "list(c.values())", "items": "list(c.items())", "copy": "c.copy()", "clear": "c.clear()",
        "fromkeys": "dict.fromkeys(c, 0)", "or": "c | {'a': 0}", "ior": "c |= {'k': 1}\nlen(c)", "in": "'a' in c", "len": "len(c)",
        "set_item": "c['a'] = 5\nc['a']", "del_item": "del c['a']\nlen(c)", "iter": "list(c)", "sorted": "sorted(c)",
        "star": "{**c, 'x': 1}", "eq": "c == {'a': 1, 'b': 2}", "min": "min(c)", "str": "str(c)", "bool": "bool(c)",
        "dict_of": "dict(c, extra=1)", "reversed": "list(reversed(c))", "comprehension": "{k: v for k, v in c.items() if k != 'a'}",
    },
    "set": {
        "add": "c.add(5)", "discard": "c.discard(1)", "remove": "c.remove(1)", "pop": "c.pop() in (1, 2, 3, 4, 'a')", "union": "c.union({9})",
        "inter": "c & {1, 2}", "diff": "c - {1}", "sym": "c ^ {1, 9}", "update": "c.update([7])", "isub": "c -= {1}\nlen(c)",
        "issubset": "c <= {1, 2, 3}", "in": "1 in c", "len": "len(c)", "sorted": "sorted(c)", "copy": "c.copy()", "clear": "c.clear()",
        "frozen": "frozenset(c) == c", "eq": "c == {1, 2}", "bad_op": "c + {1}", "bool": "bool(c)", "min": "min(c)", "sum": "sum(c)",
        "list_len": "len(list(c))", "isdisjoint": "c.isdisjoint([2])",
    },
    "tuple": {
        "item": "c[0]", "item_far": "c[3]", "slice": "c[::-1]", "count": "c.count(1)", "index": "c.index(1)", "index_missing": "c.index(9)",
        "add": "c + (0,)", "add_bad": "c + [0]", "mul": "c * 2", "in": "1 in c", "len": "len(c)", "sorted": "sorted(c)", "hash": "hash(c) == hash(tuple(c))",
        "unpack": "(lambda a, b: b)(*c)", "lt": "c < (2, 0)", "eq": "c == (2, 1)", "list": "list(c)", "set_item": "c[0] = 1\nc",
        "max": "max(c)", "sum": "sum(c)", "str": "str(c)", "dict": "dict([c])",
    },
}

TAIL = [
    "except TypeError:",
    "    r = 'TypeError!'",
    "except KeyError:",
    "    r = 'KeyError!'",
    "except IndexError:",
    "    r = 'IndexError!'",
    "except ValueError:",
    "    r = 'ValueError!'",
    "except AttributeError:",
    "    r = 'AttributeError!'",
    "except Exception:",
    "    r = 'Exception!'",
    "if r is None or isinstance(r, (bool, int, float, str, bytes)):",
    "    O = r",
    "else:",
    "    O = repr(r)",
    "O = repr(c)",
]


def attempt(expr: str) -> list:
    *stmts, last = expr.split("\n")
    return ["    try:", *(f"        {x}" for x in stmts), f"        r = {last}", *("    " + ln for ln in TAIL)]


def program(value: str, expr: str, desc: str, first: str | None = None) -> Prog:
    lines = ["import json", "def p{N}():", "    global O", f"    c = {value}", *(attempt(first) if first else []), *attempt(expr)]
    return Prog(lines, desc=desc)


def programs():
    for kind in RECEIVERS:
        for (rk, rv), (ok, ov) in itertools.product(RECEIVERS[kind].items(), OPS[kind].items()):
            yield program(rv, ov, f"{kind} {rk} {ok}")


# the first operation of a pair: those that change the receiver
FIRST = {
    "list": ["append", "extend", "insert", "pop", "pop0", "remove", "sort", "reverse", "clear", "iadd", "set_item", "set_slice", "del_item"],
    "dict": ["pop", "pop_missing", "popitem", "setdefault_new", "update", "update_kw", "clear", "ior", "set_item", "del_item"],
    "set": ["add", "discard", "remove", "pop", "update", "isub", "clear"],
    "tuple": ["set_item"],
}


def pair_programs():
    for kind in RECEIVERS:
        for (rk, rv), first, (ok, ov) in itertools.product(RECEIVERS[kind].items(), FIRST[kind], OPS[kind].items()):
            yield program(rv, ov, f"{kind} {rk} {first}+{ok}", OPS[kind][first])


if __name__ == "__main__":
    cli(programs, NAMES, pairs=pair_programs)
