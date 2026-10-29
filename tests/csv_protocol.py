"""The csv model (`models/csvio.rs`), checked exhaustively against CPython in the style of
`lib_protocol.py` (which has the plain reader and writer calls), on dialects, quoting and the
dict reader / writer:

- an input text (`INPUTS`): plain, quoted with embedded delimiters and quotes, tab and
  semicolon separated, spaces after the delimiter, ragged rows, a quote left open, numbers to
  convert, an escape character, empty; one of two joined by a branch; an unknown (an
  environment variable: its run's value fixed);
- one reader or writer call on it (`OPS`): `delimiter`, `quotechar`, `quoting` (MINIMAL, ALL,
  NONNUMERIC, NONE with and without `escapechar`), `skipinitialspace`, `dialect='excel-tab'`,
  `strict`, `lineterminator`; DictReader with `fieldnames`, `restkey`, `restval` and its
  `line_num`, a header naming a field twice; DictWriter with `extrasaction` ('raise' /
  'ignore' / neither), `restval`, `writeheader`, `writerows` of a DictReader; QUOTE_STRINGS /
  QUOTE_NOTNULL; numbers and booleans written; a reader over lines; `Sniffer`;
- in a `try` naming the class of what it raises (`csv.Error` is an `Exception!` here).

Observation point: the result (a scalar, a container's items, `lib_protocol.ENC`) or the class
it raised.

    uv run --no-project python tests/csv_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from lib_protocol import ENC_HELPERS, TAIL_ENC, program
from protocol_lib import cli

NAMES = ["O"]

LIB = {
    "imports": ["import csv", "import io"],
    "inputs": {
        "plain": "'a,b\\n1,2\\n'", "quoted": "'a,\"b,c\"\\n\"x\"\"y\",z\\r\\n'", "tabs": "'a\\tb\\n1\\t2\\n'", "semi": "'a;b\\n1;2\\n'",
        "spaces": "'a, b\\n1, \"2\"\\n'", "ragged": "'a,b,c\\n1\\n1,2,3,4\\n'", "open_quote": "'a,\"b\\n1,2\\n'", "numbers": "'1,2.5,x\\n\"3\",-4,\\n'",
        "escaped": "'a\\\\,b,c\\n'", "empty": "''", "joined": "'k,v\\n1,2\\n' if c1 else 'x;y\\n'",
        "unknown": "os.environ.get('RV{N}', 'h1,h2\\nv1,v2\\n')",
    },
    "ops": {
        "rows": "list(csv.reader(io.StringIO(v)))", "semi": "list(csv.reader(io.StringIO(v), delimiter=';'))",
        "tab_dialect": "list(csv.reader(io.StringIO(v), dialect='excel-tab'))", "quotechar": "list(csv.reader(io.StringIO(v), quotechar=\"'\"))",
        "nonnumeric": "list(csv.reader(io.StringIO(v), quoting=csv.QUOTE_NONNUMERIC))", "quote_none": "list(csv.reader(io.StringIO(v), quoting=csv.QUOTE_NONE))",
        "escapechar": "list(csv.reader(io.StringIO(v), escapechar='\\\\'))", "skipspace": "list(csv.reader(io.StringIO(v), skipinitialspace=True))",
        "strict": "list(csv.reader(io.StringIO(v), strict=True))", "line_num": "LINENUM(v)",
        "dict_fields": "[sorted(r.items()) for r in csv.DictReader(io.StringIO(v), fieldnames=['p', 'q'])]",
        "dict_restkey": "[r.get('rest') for r in csv.DictReader(io.StringIO(v), restkey='rest')]",
        "dict_restval": "[sorted(r.items()) for r in csv.DictReader(io.StringIO(v), restval='-')]",
        "dict_header": "csv.DictReader(io.StringIO(v)).fieldnames", "dict_first_key": "list(next(csv.DictReader(io.StringIO(v))))",
        "w_minimal": "W(v)", "w_all": "W(v, quoting=csv.QUOTE_ALL)", "w_nonnumeric": "W(v, quoting=csv.QUOTE_NONNUMERIC)",
        "w_none": "W(v, quoting=csv.QUOTE_NONE)", "w_none_esc": "W(v, quoting=csv.QUOTE_NONE, escapechar='\\\\')",
        "w_term": "W(v, lineterminator='\\n')", "w_tab": "W(v, dialect='excel-tab')", "w_quotechar": "W(v, quotechar=\"'\")",
        "dw_raise": "DW(v, 'raise')", "dw_ignore": "DW(v, 'ignore')", "dw_restval": "DWR(v)", "dw_bad_action": "DW(v, 'drop')",
        "dw_rows": "DWROWS(v)", "w_strings": "W(v, quoting=csv.QUOTE_STRINGS)", "w_notnull": "W(v, quoting=csv.QUOTE_NOTNULL)",
        "lines": "list(csv.reader(v.splitlines()))", "next": "next(csv.reader(io.StringIO(v)))", "dup_header": "DUPH(v)",
        "sniff": "csv.Sniffer().sniff(v).delimiter", "has_header": "csv.Sniffer().has_header(v)", "w_floats": "WF(v)",
        "dict_len": "len(list(csv.DictReader(io.StringIO(v))))", "dict_values": "[list(r.values()) for r in csv.DictReader(io.StringIO(v))]",
    },
    "helpers": [
        "def W(v, **kw):", "    f = io.StringIO()", "    w = csv.writer(f, **kw)", "    w.writerow([v, 1, 2.5, None, ''])", "    return f.getvalue()",
        "def DW(v, action):", "    f = io.StringIO()", "    w = csv.DictWriter(f, fieldnames=['k', 'n'], extrasaction=action)",
        "    w.writeheader()", "    w.writerow({'k': v, 'n': 1, 'extra': 2})", "    return f.getvalue()",
        "def DWR(v):", "    f = io.StringIO()", "    w = csv.DictWriter(f, fieldnames=['k', 'n'], restval='?')", "    w.writerow({'k': v})",
        "    return f.getvalue()",
        "def DWROWS(v):", "    f = io.StringIO()", "    w = csv.DictWriter(f, fieldnames=['a', 'b'])",
        "    w.writerows(csv.DictReader(io.StringIO(v)))", "    return f.getvalue()",
        "def LINENUM(v):", "    r = csv.reader(io.StringIO(v))", "    n = 0", "    for _ in r:", "        n += 1", "    return [n, r.line_num]",
        "def DUPH(v):", "    rows = list(csv.DictReader(io.StringIO('a,a,' + v)))", "    return [sorted(r.items()) for r in rows]",
        "def WF(v):", "    f = io.StringIO()", "    csv.writer(f).writerow([0.1 + 0.2, len(v), True, v.upper()])", "    return f.getvalue()",
        *ENC_HELPERS,
    ],
}


def programs():
    for (ik, iv), (ok, ov) in itertools.product(LIB["inputs"].items(), LIB["ops"].items()):
        yield program(LIB, iv, ov, f"csv {ik} {ok}", TAIL_ENC)


if __name__ == "__main__":
    cli(programs, NAMES)
