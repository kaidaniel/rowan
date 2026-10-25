"""The library models, checked exhaustively against CPython (AGENTS.md, Models: json, re,
datetime, decimal, csv; `models/README.md`):

- an input (`INPUTS` per library): literals, two literals joined by a branch, and unknowns
  (`os.environ.get(...)`: a typed `str(?)`, its run's value fixed);
- one call or a short chain of them on it (`OPS` per library), with arguments of the wrong
  type or value too (`json.dumps(set())`, `re.compile('[')`, `date(2024, 2, 30)`,
  `Decimal('x')`, a csv quote left open);
- in a `try` naming the class of what it raises.

Observation point: the result (a scalar, else its `repr`) or the class it raised.

    uv run --no-project python tests/lib_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O"]

LIBS = {
    "json": {
        "imports": ["import json"],
        "inputs": {
            "none": "None", "true": "True", "int": "1", "float": "1.5", "str": "'a'", "nonascii": "'é\\n'",
            "list": "[1, 'a', None]", "dict": "{'b': [1, 2], 'a': {'c': True}}", "intkeys": "{1: 'x', 2.5: 'y', None: 0}",
            "tuple": "(1, 2)", "nan": "float('nan')", "set": "{1}", "tuplekey": "{(1, 2): 3}", "bytes": "b'x'",
            "joined": "[1] if c1 else {'k': 'v'}", "unknown": "os.environ.get('RJ{N}', 'x')",
            "text_list": "'[1, 2.5, \"a\"]'", "text_obj": "' {\"a\": [true, null], \"b\": 1e400} '", "text_dup": "'{\"a\": 1, \"a\": 2}'",
            "text_bad": "'[1,]'", "text_empty": "''", "text_nan": "'NaN'", "text_unknown": "os.environ.get('RT{N}', '{\"k\": [1]}')",
        },
        "ops": {
            "dumps": "json.dumps(v)", "indent": "json.dumps(v, indent=2)", "sort": "json.dumps(v, sort_keys=True)",
            "seps": "json.dumps(v, separators=(',', ':'))", "ascii": "json.dumps(v, ensure_ascii=False)",
            "default": "json.dumps(v, default=str)", "nan_strict": "json.dumps(v, allow_nan=False)",
            "skipkeys": "json.dumps(v, skipkeys=True)", "loads": "json.loads(v)", "loads_key": "json.loads(v)['a']",
            "loads_type": "type(json.loads(v)).__name__", "roundtrip": "json.loads(json.dumps(v))",
            "loads_len": "len(json.loads(v))",
        },
    },
    "re": {
        "imports": ["import re"],
        "inputs": {
            "empty": "''", "dashes": "'aa-12-34'", "ab": "'ab'", "commas": "'x,y , z'", "upper": "'AB'",
            "lines": "'a1\\nb2'", "joined": "'ab' if c1 else '9-9'", "unknown": "os.environ.get('RR{N}', 'k-1')",
        },
        "ops": {
            "match": "re.match(r'a+', v)", "match_g": "re.match(r'a+', v).group(0)", "search_groups": "re.search(r'(\\d+)-(\\d+)', v).groups()",
            "fullmatch": "re.fullmatch(r'\\w+', v) is not None", "findall": "re.findall(r'[a-z]', v)", "findall_g": "re.findall(r'(\\w)(\\d)', v)",
            "sub": "re.sub(r'\\d', '#', v)", "sub_fn": "re.sub(r'[a-z]', lambda m: m.group(0).upper(), v)", "subn": "re.subn(r'a', 'b', v)",
            "split": "re.split(r'\\s*,\\s*', v)", "split_g": "re.split(r'(-)', v)", "named": "re.search(r'(?P<k>[a-z])', v).group('k')",
            "named_dict": "re.search(r'(?P<k>[a-z])(?P<d>\\d)?', v).groupdict()", "optional": "re.match(r'(a)?b', v).group(1)",
            "span": "re.search(r'\\d+', v).span()", "start": "re.search(r'-', v).start()", "lazy": "re.match(r'.*?', v).end()",
            "ignorecase": "re.match(r'(?i)ab', v) is not None", "flags": "re.findall(r'^\\w', v, re.M)", "bad": "re.compile(r'[')",
            "escape": "re.escape(v)", "compiled": "re.compile(r'a|b').findall(v)", "pattern": "re.compile(v).pattern",
            "group_far": "re.match(r'a', v).group(2)", "finditer": "[m.span() for m in re.finditer(r'\\d', v)]",
        },
    },
    "datetime": {
        "imports": ["import datetime as dt"],
        "inputs": {
            "date": "'2024-01-31'", "datetime": "'2024-02-29T23:59:30'", "tz": "'2024-03-01T00:00:00+01:00'", "bad_month": "'2024-13-01'",
            "bad": "'x'", "space": "'2023-12-31 08:15'", "joined": "'2024-01-31' if c1 else '2023-06-15'",
            "unknown": "os.environ.get('RD{N}', '2021-07-04')",
        },
        "ops": {
            "fromiso": "dt.datetime.fromisoformat(v)", "iso": "dt.datetime.fromisoformat(v).isoformat()",
            "date_fromiso": "dt.date.fromisoformat(v)", "plus_day": "dt.date.fromisoformat(v[:10]) + dt.timedelta(days=1)",
            "plus_month": "dt.date.fromisoformat(v[:10]).replace(month=2)", "weekday": "dt.date.fromisoformat(v[:10]).weekday()",
            "isoweekday": "dt.date.fromisoformat(v[:10]).isocalendar()", "strftime": "dt.datetime.fromisoformat(v).strftime('%Y/%m/%d %H:%M %a %j')",
            "strptime": "dt.datetime.strptime(v, '%Y-%m-%d')", "strptime_t": "dt.datetime.strptime(v[:10], '%Y-%m-%d').date()",
            "diff": "dt.datetime.fromisoformat(v) - dt.datetime(2024, 1, 1)", "diff_days": "(dt.date.fromisoformat(v[:10]) - dt.date(2020, 1, 1)).days",
            "td": "dt.timedelta(hours=25, seconds=v.count('-'))", "td_parts": "dt.timedelta(minutes=-len(v)).seconds",
            "total": "dt.timedelta(days=len(v)).total_seconds()", "date_bad": "dt.date(2024, 2, 30 - len(v) // 99)",
            "cmp": "dt.date.fromisoformat(v[:10]) < dt.date(2024, 1, 1)", "ts": "dt.datetime(2024, 1, 2, tzinfo=dt.timezone.utc).timestamp()",
            "year": "dt.datetime.fromisoformat(v).year", "time": "dt.time.fromisoformat(v[11:16])", "str": "str(dt.date.fromisoformat(v[:10]))",
        },
    },
    "decimal": {
        "imports": ["from decimal import Decimal, InvalidOperation", "import decimal"],
        "inputs": {
            "one": "'1.10'", "big": "'1e3'", "neg": "'-2.675'", "zero": "'0.000'", "bad": "'x'", "nan": "'NaN'", "inf": "'-Infinity'",
            "int": "'7'", "joined": "'2.5' if c1 else '3'", "unknown": "os.environ.get('RM{N}', '4.20')",
        },
        "ops": {
            "str": "str(Decimal(v))", "repr": "repr(Decimal(v))", "add": "Decimal(v) + Decimal('2.205')", "mul": "Decimal(v) * 3",
            "div": "Decimal(v) / Decimal('3')", "floordiv": "Decimal(v) // 2", "quantize": "Decimal(v).quantize(Decimal('0.1'))",
            "round": "round(Decimal(v), 2)", "int": "int(Decimal(v))", "float": "float(Decimal(v))", "eq": "Decimal(v) == Decimal('1.1')",
            "lt": "Decimal(v) < 2", "sqrt": "Decimal(v).sqrt()", "neg": "-Decimal(v)", "abs": "abs(Decimal(v))",
            "normalize": "Decimal(v).normalize()", "tuple": "Decimal(v).as_tuple()", "from_float": "Decimal(1.1) + Decimal(v)",
            "fmt": "f'{Decimal(v):.2f}'", "is_nan": "Decimal(v).is_nan()", "add_float": "Decimal(v) + 1.5",
        },
    },
    "csv": {
        "imports": ["import csv", "import io"],
        "inputs": {
            "plain": "'a,b\\n1,2\\n'", "quoted": "'a,\"b,c\"\\n\"x\"\"y\",z\\r\\n'", "semi": "'a;b\\n1;2'", "open_quote": "'\"abc\\n'",
            "empty": "''", "blank_lines": "'a,b\\r\\n\\r\\n1\\n'", "ragged": "'a,b\\n1\\n1,2,3\\n'", "joined": "'k,v\\n1,2' if c1 else 'x\\n'",
            "unknown": "os.environ.get('RC{N}', 'h1,h2\\nv1,v2\\n')",
        },
        "ops": {
            "reader": "list(csv.reader(io.StringIO(v)))", "semi": "list(csv.reader(io.StringIO(v), delimiter=';'))",
            "strict": "list(csv.reader(io.StringIO(v), strict=True))", "first": "next(csv.reader(io.StringIO(v)))",
            "dict": "list(csv.DictReader(io.StringIO(v)))", "fields": "csv.DictReader(io.StringIO(v)).fieldnames",
            "dict_key": "next(csv.DictReader(io.StringIO(v)))['a']", "count": "sum(1 for _ in csv.reader(io.StringIO(v)))",
            "lines": "list(csv.reader(v.splitlines()))", "write": "W(v)", "write_all": "W(v, quoting=csv.QUOTE_ALL)",
            "write_semi": "W(v, delimiter=';')", "write_dict": "WD(v)",
        },
        "helpers": [
            "def W(v, **kw):",
            "    f = io.StringIO()",
            "    w = csv.writer(f, **kw)",
            "    w.writerow([v, 1.5, None, 'a\"b'])",
            "    w.writerows([[v], []])",
            "    return f.getvalue()",
            "def WD(v):",
            "    f = io.StringIO()",
            "    w = csv.DictWriter(f, fieldnames=['k', 'v'])",
            "    w.writeheader()",
            "    w.writerow({'k': v, 'v': 2})",
            "    w.writerow({'k': 1})",
            "    return f.getvalue()",
        ],
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
    "except ArithmeticError:",
    "    r = 'ArithmeticError!'",
    "except Exception:",
    "    r = 'Exception!'",
    "if r is None or isinstance(r, (bool, int, float, str, bytes)):",
    "    O = r",
    "else:",
    "    O = repr(r)",
]


def program(lib: dict, value: str, expr: str, desc: str) -> Prog:
    helpers = [ln.replace("def W", "def W{N}_").replace("WD(", "WD{N}_(") for ln in lib.get("helpers", [])]
    expr = expr.replace("WD(", "WD{N}_(").replace("W(", "W{N}_(")
    lines = [*lib["imports"], *helpers, "def p{N}():", "    global O", f"    v = {value}", "    try:", f"        r = {expr}"]
    lines += ["    " + ln for ln in TAIL]
    return Prog(lines, desc=desc)


def programs():
    for name, lib in LIBS.items():
        for (ik, iv), (ok, ov) in itertools.product(lib["inputs"].items(), lib["ops"].items()):
            yield program(lib, iv, ov, f"{name} {ik} {ok}")


if __name__ == "__main__":
    cli(programs, NAMES)
