"""The collections models, checked exhaustively against CPython (AGENTS.md, Models;
`models/counter.rs`, `stdlib.rs`: Counter, defaultdict, OrderedDict, deque, namedtuple), in the
style of `lib_protocol.py`:

- an input (`INPUTS` per type): literals, two joined by a branch, an unknown (`json.loads` of
  an environment variable: its run's value fixed);
- one construction and an operation on it (`OPS`), with wrong keys, indices and arguments too
  (a missing key, an empty deque's `pop`, a namedtuple of the wrong arity);
- in a `try` naming the class of what it raises.

Observation point: the result (a scalar, else its `repr`) or the class it raised.

    uv run --no-project python tests/collections_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import program
from protocol_lib import cli

NAMES = ["O"]

LIBS = {
    "Counter": {
        "imports": ["import json", "from collections import Counter"],
        "inputs": {
            "text": "'abca'", "list": "['x', 'y', 'x']", "map": "{'a': 2, 'b': -1}", "empty": "''",
            "joined": "'ab' if c1 else 'bb'", "unknown": "json.loads(os.environ.get('RC{N}', '\"aab\"'))",
        },
        "ops": {
            "item": "Counter(v)['a']", "missing": "Counter(v)['z']", "common": "Counter(v).most_common(1)",
            "common_all": "Counter(v).most_common()", "elements": "sorted(Counter(v).elements())", "total": "Counter(v).total()",
            "len": "len(Counter(v))", "add": "sorted((Counter(v) + Counter('a')).items())", "sub": "sorted((Counter(v) - Counter('a')).items())",
            "and": "sorted((Counter(v) & Counter('ab')).items())", "or": "sorted((Counter(v) | Counter('c')).items())",
            "neg": "sorted((-Counter(v)).items())", "pos": "sorted((+Counter(v)).items())", "update": "UPD(v)",
            "get": "Counter(v).get('a', 0)", "dict": "sorted(dict(Counter(v)).items())", "keys": "list(Counter(v))",
            "eq": "Counter(v) == Counter('aab')", "in": "'a' in Counter(v)", "last": "Counter(v).most_common()[-1]",
            "del": "DEL(v)", "kw": "sorted(Counter(v, z=3).items())", "values": "sorted(Counter(v).values())",
        },
        "helpers": [
            "def UPD(v):", "    c = Counter(v)", "    c.update('zz')", "    c.subtract('a')", "    return sorted(c.items())",
            "def DEL(v):", "    c = Counter(v)", "    del c['a']", "    return len(c)",
        ],
    },
    "defaultdict": {
        "imports": ["import json", "from collections import defaultdict"],
        "inputs": {
            "pairs": "[('a', 1), ('b', 2), ('a', 3)]", "empty": "[]", "one": "[('x', 0)]",
            "joined": "[('a', 1)] if c1 else [('b', 2), ('b', 3)]", "unknown": "json.loads(os.environ.get('RD{N}', '[[\"a\", 5]]'))",
        },
        "ops": {
            "item": "DD(v)['a']", "new": "DD(v)['zz']", "len": "len(DD(v))", "keys": "sorted(DD(v))", "get": "DD(v).get('q')",
            "in": "'zz' in DD(v)", "count": "sorted(DDI(v).items())", "none": "defaultdict(None)['k']", "init": "defaultdict(int, {'a': 1})['a']",
            "factory": "DD(v).default_factory is list", "dict": "sorted(dict(DD(v)).items())", "pop": "DD(v).pop('a', None)",
            "setdefault": "DD(v).setdefault('n', 5)", "after_new": "AFTER(v)", "bad_factory": "defaultdict(5)",
        },
        "helpers": [
            "def DD(v):", "    d = defaultdict(list)", "    for k, x in v:", "        d[k].append(x)", "    return d",
            "def DDI(v):", "    d = defaultdict(int)", "    for k, x in v:", "        d[k] += x", "    return d",
            "def AFTER(v):", "    d = DD(v)", "    d['new']", "    return sorted(d)",
        ],
    },
    "OrderedDict": {
        "imports": ["import json", "from collections import OrderedDict"],
        "inputs": {
            "pairs": "[('a', 1), ('b', 2), ('c', 3)]", "empty": "[]", "dup": "[('x', 1), ('x', 2)]",
            "joined": "[('a', 1)] if c1 else [('b', 2), ('a', 3)]", "unknown": "json.loads(os.environ.get('RO{N}', '[[\"k\", 1]]'))",
        },
        "ops": {
            "keys": "list(OrderedDict(v))", "move": "MOVE(v)", "move_first": "MOVEF(v)", "popitem": "OrderedDict(v).popitem()",
            "popitem_first": "OrderedDict(v).popitem(last=False)", "eq_order": "OrderedDict(v) == OrderedDict(list(reversed(v)))",
            "eq_dict": "OrderedDict(v) == dict(v)", "item": "OrderedDict(v)['a']", "len": "len(OrderedDict(v))",
            "reversed": "list(reversed(OrderedDict(v)))", "move_missing": "OrderedDict(v).move_to_end('zz')", "values": "list(OrderedDict(v).values())",
            "setitem": "SET(v)",
        },
        "helpers": [
            "def MOVE(v):", "    d = OrderedDict(v)", "    d.move_to_end('a')", "    return list(d)",
            "def MOVEF(v):", "    d = OrderedDict(v)", "    d.move_to_end('b', last=False)", "    return list(d)",
            "def SET(v):", "    d = OrderedDict(v)", "    d['a'] = 0", "    d['n'] = 1", "    return list(d.items())",
        ],
    },
    "deque": {
        "imports": ["import json", "from collections import deque"],
        "inputs": {
            "ints": "[1, 2, 3]", "empty": "[]", "text": "'ab'", "joined": "[1] if c1 else [2, 3]",
            "unknown": "json.loads(os.environ.get('RQ{N}', '[4, 5]'))",
        },
        "ops": {
            "popleft": "deque(v).popleft()", "pop": "deque(v).pop()", "maxlen2": "list(deque(v, maxlen=2))", "ops": "DQ(v)",
            "first": "deque(v)[0]", "last": "deque(v)[-1]", "len": "len(deque(v))", "maxlen": "deque(v).maxlen", "maxlen_set": "deque(v, 2).maxlen",
            "reversed": "list(reversed(deque(v)))", "count": "deque(v).count(1)", "index": "deque(v).index(2)", "extend": "DQX(v)",
            "in": "1 in deque(v)", "eq": "deque(v) == deque([1, 2, 3])", "rotate": "ROT(v)", "remove": "REM(v)", "clear": "CLR(v)",
            "bad_maxlen": "deque(v, -1)", "full": "FULL(v)",
        },
        "helpers": [
            "def DQ(v):", "    d = deque(v)", "    d.appendleft(0)", "    d.append(9)", "    return list(d)",
            "def DQX(v):", "    d = deque(v)", "    d.extend('xy')", "    d.extendleft([7, 8])", "    return list(d)",
            "def ROT(v):", "    d = deque(v)", "    d.rotate(1)", "    return list(d)",
            "def REM(v):", "    d = deque(v)", "    d.remove(2)", "    return list(d)",
            "def CLR(v):", "    d = deque(v)", "    d.clear()", "    return len(d)",
            "def FULL(v):", "    d = deque(v, maxlen=2)", "    d.append(9)", "    d.appendleft(8)", "    return list(d)",
        ],
    },
    "namedtuple": {
        "imports": ["import json", "from collections import namedtuple", "P = namedtuple('P', 'x y')", "Q = namedtuple('Q', ['a', 'b'], defaults=[7])"],
        "inputs": {
            "ints": "(1, 2)", "mixed": "('a', None)", "joined": "(1, 2) if c1 else (3, 4)",
            "unknown": "tuple(json.loads(os.environ.get('RN{N}', '[5, 6]')))", "short": "(1,)",
        },
        "ops": {
            "x": "P(*v).x", "item": "P(*v)[1]", "asdict": "P(*v)._asdict()", "replace": "P(*v)._replace(x=9)", "fields": "P._fields",
            "eq_tuple": "P(*v) == tuple(v)", "len": "len(P(*v))", "missing": "P(*v).z", "extra": "P(*v, 3)", "make": "P._make(v)",
            "repr": "repr(P(*v))", "hash": "hash(P(*v)) == hash(tuple(v))", "asdict_y": "P(*v)._asdict()['y']", "default": "Q(v[0]).b",
            "kw": "P(y=v[-1], x=0)", "unpack": "(lambda a, b: b)(*P(*v))", "set": "SETX(v)", "defaults": "Q._field_defaults",
            "index": "P(*v).index(v[0])", "add": "P(*v) + (0,)",
        },
        "helpers": ["def SETX(v):", "    p = P(*v)", "    p.x = 5", "    return p.x"],
    },
}


def programs():
    for name, lib in LIBS.items():
        for (ik, iv), (ok, ov) in itertools.product(lib["inputs"].items(), lib["ops"].items()):
            yield program(lib, iv, ov, f"{name} {ik} {ok}")


if __name__ == "__main__":
    cli(programs, NAMES)
