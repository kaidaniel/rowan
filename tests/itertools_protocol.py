"""The itertools model, checked exhaustively against CPython (AGENTS.md, Models;
`models/itertools.rs`), in the style of `lib_protocol.py`:

- an input (`INPUTS`): lists of ints (unsorted, with runs, empty, one item), a str, two joined
  by a branch, and an unknown (`json.loads` of an environment variable: its run's value fixed);
- one itertools call on it, its items taken (`OPS`): chain, islice, accumulate, permutations,
  combinations, product, groupby, zip_longest, takewhile / dropwhile, compress, starmap,
  count / cycle / repeat, pairwise, batched, filterfalse, tee, with wrong arguments too
  (`islice(v, -1)`, `batched(v, 0)`, `next` of an empty cycle);
- in a `try` naming the class of what it raises.

Observation point: the result (a scalar, else its `repr`) or the class it raised.

    uv run --no-project python tests/itertools_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import program
from protocol_lib import cli

NAMES = ["O"]

LIB = {
    "imports": ["import itertools", "import json", "import operator"],
    "inputs": {
        "ints": "[3, 1, 2]", "runs": "[1, 1, 2, 3, 3]", "empty": "[]", "one": "[5]", "text": "'aab'",
        "joined": "[1, 2] if c1 else [2, 2, 4]", "unknown": "json.loads(os.environ.get('RI{N}', '[2, 7]'))",
    },
    "ops": {
        "chain": "list(itertools.chain(v, [9]))", "chain_from": "list(itertools.chain.from_iterable([v, v]))",
        "islice": "list(itertools.islice(v, 2))", "islice_from": "list(itertools.islice(v, 1, None))",
        "islice_step": "list(itertools.islice(v, 0, None, 2))", "islice_bad": "list(itertools.islice(v, -1))",
        "accumulate": "list(itertools.accumulate(v))", "accumulate_max": "list(itertools.accumulate(v, max))",
        "accumulate_init": "list(itertools.accumulate(v, operator.mul, initial=1))",
        "permutations": "list(itertools.permutations(v, 2))", "combinations": "list(itertools.combinations(v, 2))",
        "combinations_r": "list(itertools.combinations_with_replacement(v, 2))", "product": "list(itertools.product(v, 'xy'))",
        "product_repeat": "len(list(itertools.product(v, repeat=2)))", "groupby": "[(k, len(list(g))) for k, g in itertools.groupby(v)]",
        "groupby_key": "[k for k, _ in itertools.groupby(v, key=lambda x: x % 2 if isinstance(x, int) else x)]",
        "zip_longest": "list(itertools.zip_longest(v, 'xy'))", "zip_longest_fill": "list(itertools.zip_longest(v, [0], fillvalue=-1))",
        "takewhile": "list(itertools.takewhile(lambda x: x != 2, v))", "dropwhile": "list(itertools.dropwhile(lambda x: x != 2, v))",
        "compress": "list(itertools.compress(v, [1, 0, 1]))", "starmap": "list(itertools.starmap(pow, zip(v, [2, 2, 2])))",
        "count": "list(zip(itertools.count(5), v))", "count_step": "list(zip(v, itertools.count(1, 2)))",
        "cycle": "list(itertools.islice(itertools.cycle(v), 4))", "cycle_next": "next(itertools.cycle(v))",
        "repeat": "list(itertools.repeat(v, 2))", "repeat_zip": "list(zip(v, itertools.repeat('r')))",
        "pairwise": "list(itertools.pairwise(v))", "batched": "list(itertools.batched(v, 2))", "batched_bad": "list(itertools.batched(v, 0))",
        "filterfalse": "list(itertools.filterfalse(None, v))", "filterfalse_fn": "list(itertools.filterfalse(lambda x: x == 1, v))",
        "tee": "TEE(v)", "next_islice": "next(itertools.islice(v, 1, 2))", "sum_chain": "sum(itertools.chain(v, v))",
        "sorted_perm": "sorted(itertools.permutations(v))[0]", "len_comb": "len(list(itertools.combinations(v, 3)))",
    },
    "helpers": ["def TEE(v):", "    a, b = itertools.tee(v)", "    first = list(a)", "    return first, list(b)"],
}


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        yield program(LIB, iv, ov, f"itertools {ik} {ok}")


if __name__ == "__main__":
    cli(programs, NAMES)
