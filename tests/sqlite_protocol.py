"""The sqlite3 model, checked exhaustively against CPython (AGENTS.md, Models;
`models/sqlite.rs`: an in-memory connection simulating simple SQL), in the style of
`lib_protocol.py`:

- the rows a table `t (a, b)` is filled with (`INPUTS`): known rows, none, a NULL, a branch
  between two sets, and rows not known (`json.loads` of an environment variable);
- one query or statement on it (`OPS`): SELECT of columns, `*`, WHERE with a literal or a
  parameter, ORDER BY, LIMIT, count / sum, fetchone / fetchmany / iteration, `sqlite3.Row`
  rows, UPDATE / DELETE / INSERT with rowcount and lastrowid, `description`,
  `executescript`, and errors (a missing column or table, a wrong number of parameters);
- in a `try` naming the class of what it raises (sqlite3's errors: `Exception!`).

Observation point: the result (a scalar, else its `repr`) or the class it raised.

    uv run --no-project python tests/sqlite_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import program
from protocol_lib import cli

NAMES = ["O"]

LIB = {
    "imports": ["import json", "import sqlite3"],
    "inputs": {
        "two": "[(1, 'x'), (2, 'y')]", "none": "[]", "null": "[(None, 'z')]", "dup": "[(3, 'x'), (1, 'x')]",
        "joined": "[(1, 'x')] if c1 else [(2, 'y'), (3, 'y')]", "unknown": "json.loads(os.environ.get('RS{N}', '[[4, \"w\"]]'))",
    },
    "ops": {
        "sel_a": "MK(v).execute('SELECT a FROM t').fetchall()",
        "sel_order": "MK(v).execute('SELECT a, b FROM t ORDER BY a DESC').fetchall()",
        "where_param": "MK(v).execute('SELECT b FROM t WHERE a = ?', (2,)).fetchall()",
        "where_lit": "MK(v).execute(\"SELECT a FROM t WHERE b = 'x'\").fetchall()",
        "star_limit": "MK(v).execute('SELECT * FROM t LIMIT 1').fetchall()",
        "count": "MK(v).execute('SELECT count(*) FROM t').fetchone()[0]",
        "sum": "MK(v).execute('SELECT sum(a) FROM t').fetchone()[0]",
        "bad_col": "MK(v).execute('SELECT c FROM t').fetchall()",
        "bad_table": "MK(v).execute('SELECT a FROM u').fetchall()",
        "fetchone": "MK(v).execute('SELECT a FROM t ORDER BY a').fetchone()",
        "fetchone_0": "MK(v).execute('SELECT a FROM t ORDER BY a').fetchone()[0]",
        "fetchmany": "MK(v).execute('SELECT b FROM t').fetchmany(1)",
        "iter": "[r[0] for r in MK(v).execute('SELECT a FROM t')]",
        "row": "ROW(v)", "row_key": "ROWK(v)", "row_bad_key": "ROWBAD(v)",
        "update": "UPD(v)", "rowcount": "RC(v)", "delete": "DEL(v)", "lastrowid": "LAST(v)",
        "description": "[d[0] for d in MK(v).execute('SELECT a, b FROM t').description]",
        "bad_params": "MK(v).execute('SELECT a FROM t WHERE a = ?', (1, 2)).fetchall()",
        "script": "SCRIPT(v)", "order_b": "MK(v).execute('SELECT b FROM t ORDER BY b').fetchall()",
        "len": "len(MK(v).execute('SELECT a, b FROM t').fetchall())",
        # (a table filled with literal rows, which the model simulates: the same queries)
        "lit_sel": "LIT().execute('SELECT a FROM t').fetchall()", "lit_order": "LIT().execute('SELECT a, b FROM t ORDER BY a DESC').fetchall()",
        "lit_where": "LIT().execute('SELECT b FROM t WHERE a = ?', (v[0][0] if v else 2,)).fetchall()",
        "lit_where_lit": "LIT().execute(\"SELECT a FROM t WHERE b = 'x'\").fetchall()", "lit_limit": "LIT().execute('SELECT * FROM t LIMIT 1').fetchall()",
        "lit_count": "LIT().execute('SELECT count(*) FROM t').fetchone()[0]", "lit_sum": "LIT().execute('SELECT sum(a) FROM t').fetchone()[0]",
        "lit_bad_col": "LIT().execute('SELECT c FROM t').fetchall()", "lit_one": "LIT().execute('SELECT a FROM t ORDER BY a').fetchone()",
        "lit_row": "LITROW()", "lit_update": "LITUPD()", "lit_delete": "LITDEL()", "lit_null": "LIT().execute('SELECT a FROM t WHERE a IS NULL').fetchall()",
        "lit_desc": "[d[0] for d in LIT().execute('SELECT b, a FROM t').description]", "lit_iter": "[r[1] for r in LIT().execute('SELECT a, b FROM t')]",
    },
    "helpers": [
        "def MK(v):", "    con = sqlite3.connect(':memory:')", "    con.execute('CREATE TABLE t (a, b)')",
        "    con.executemany('INSERT INTO t VALUES (?, ?)', v)", "    return con",
        "def ROW(v):", "    con = MK(v)", "    con.row_factory = sqlite3.Row", "    r = con.execute('SELECT a, b FROM t').fetchone()",
        "    return dict(r) if r else None",
        "def ROWK(v):", "    con = MK(v)", "    con.row_factory = sqlite3.Row", "    rows = con.execute('SELECT a, b FROM t').fetchall()",
        "    return [r['b'] for r in rows]",
        "def ROWBAD(v):", "    con = MK(v)", "    con.row_factory = sqlite3.Row", "    rows = con.execute('SELECT a FROM t').fetchall()",
        "    return [r['b'] for r in rows]",
        "def UPD(v):", "    con = MK(v)", "    con.execute(\"UPDATE t SET b = 'q' WHERE a = 1\")",
        "    return con.execute('SELECT b FROM t ORDER BY a').fetchall()",
        "def RC(v):", "    con = MK(v)", "    return con.execute(\"UPDATE t SET b = 'q' WHERE a < 3\").rowcount",
        "def DEL(v):", "    con = MK(v)", "    con.execute('DELETE FROM t WHERE a > 1')", "    return con.execute('SELECT count(*) FROM t').fetchone()[0]",
        "def LAST(v):", "    con = MK(v)", "    return con.execute(\"INSERT INTO t VALUES (9, 'n')\").lastrowid",
        "def LIT():", "    con = sqlite3.connect(':memory:')", "    con.execute('CREATE TABLE t (a, b)')",
        "    con.execute(\"INSERT INTO t VALUES (1, 'x')\")", "    con.execute(\"INSERT INTO t VALUES (2, 'y')\")",
        "    con.execute(\"INSERT INTO t VALUES (NULL, 'x')\")", "    return con",
        "def LITROW():", "    con = LIT()", "    con.row_factory = sqlite3.Row", "    return [dict(r) for r in con.execute('SELECT a, b FROM t WHERE a > 1')]",
        "def LITUPD():", "    con = LIT()", "    n = con.execute(\"UPDATE t SET b = 'q' WHERE a = 1\").rowcount",
        "    return n, con.execute('SELECT b FROM t').fetchall()",
        "def LITDEL():", "    con = LIT()", "    con.execute('DELETE FROM t WHERE a > 1')", "    return con.execute('SELECT a FROM t').fetchall()",
        "def SCRIPT(v):", "    con = MK(v)", "    con.executescript('CREATE TABLE u (c); INSERT INTO u VALUES (5);')",
        "    return con.execute('SELECT c FROM u').fetchall()",
    ],
}


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        yield program(LIB, iv, ov, f"sqlite {ik} {ok}")


if __name__ == "__main__":
    cli(programs, NAMES)
