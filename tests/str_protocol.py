"""The str model and formatting, checked exhaustively against CPython (AGENTS.md, Models: str,
formatting; Builtins: `kv_of_type`, TypeErrors from builtins given wrong types):

- a receiver `s` (`RECEIVERS`): str literals (empty, letters, cases, spaces and tabs,
  separators, digits, a sign, non-ASCII), two literals joined by a branch, and str unknowns
  (`os.environ.get(...)`: a typed `str(?)`, its run's value fixed);
- one operation on it (`OPS`): every str method with literal arguments, including arguments of
  the wrong type or value (`s.split('')`, `s.startswith(1)`, `s.join([1])`), subscripts and
  slices, operators, `in`, `len` / `int` / `float` / `ord` / `repr`, and formatting of `s` and
  of literal and unknown numbers with `%`, `.format`, f-string specs and `format` (`FORMATS`),
  wrong types and specs included;
- in a `try` naming the class of what it raises.

Observation point: the result (a scalar, else its `repr`) or the class it raised.

    uv run --no-project python tests/str_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O"]

RECEIVERS = {
    "empty": "''",
    "a": "'a'",
    "words": "'Ab c'",
    "spaces": "' x\\t'",
    "csv": "'a,b,,c'",
    "digits": "'12'",
    "neg": "'-3'",
    "title": "'Hello World'",
    "sharp": "'ß'",
    "lines": "'a\\nb\\r\\nc'",
    "sep_ws": "'\\x1ca b\\x1f'",
    "cjk": "'中a b'",
    "digraph": "'ǆx'",
    "dotted": "'İa'",
    "sigma": "'ΑΣ Σ'",
    "superscript": "'²3'",
    "arabic": "'٣4'",
    "under": "'1_000'",
    "float_txt": "' 1e3 '",
    "nan": "'nan'",
    "joined": "'ab' if c1 else 'A b'",
    "unknown": "os.environ.get('RS{N}', 'a,B c')",
    "unknown_num": "os.environ.get('RN{N}', '42')",
    "unknown_b": "B",
}

OPS = {
    # case and spacing
    "upper": "s.upper()", "lower": "s.lower()", "casefold": "s.casefold()", "title": "s.title()",
    "capitalize": "s.capitalize()", "swapcase": "s.swapcase()",
    "strip": "s.strip()", "lstrip": "s.lstrip()", "rstrip": "s.rstrip()", "strip_chars": "s.strip('a ')",
    "strip_bad": "s.strip(1)",
    "center": "s.center(6, '*')", "ljust": "s.ljust(4)", "rjust": "s.rjust(4, '-')", "zfill": "s.zfill(4)",
    "center_bad": "s.center(4, 'ab')", "expandtabs": "s.expandtabs(2)",
    # predicates
    "isdigit": "s.isdigit()", "isalpha": "s.isalpha()", "isalnum": "s.isalnum()", "isspace": "s.isspace()",
    "isupper": "s.isupper()", "islower": "s.islower()", "istitle": "s.istitle()", "isdecimal": "s.isdecimal()",
    "isnumeric": "s.isnumeric()", "isidentifier": "s.isidentifier()", "isascii": "s.isascii()",
    "isprintable": "s.isprintable()",
    "startswith": "s.startswith('a')", "startswith_tuple": "s.startswith(('x', 'A'))", "endswith": "s.endswith('c')",
    "startswith_bad": "s.startswith(1)", "startswith_pos": "s.startswith('b', 2)",
    # searching
    "find": "s.find('b')", "rfind": "s.rfind(',')", "index": "s.index('b')", "rindex": "s.rindex('a')",
    "count": "s.count(',')", "count_empty": "s.count('')", "in": "'b' in s", "in_bad": "1 in s",
    # splitting and joining
    "split": "s.split()", "split_sep": "s.split(',')", "split_max": "s.split(',', 1)", "rsplit": "s.rsplit(',', 1)",
    "split_empty": "s.split('')", "splitlines": "s.splitlines()", "splitlines_keep": "s.splitlines(True)",
    "partition": "s.partition(',')", "rpartition": "s.rpartition(' ')",
    "join": "s.join(['x', 'y'])", "join_bad": "s.join(['x', 1])", "join_str": "'-'.join(s)",
    # replacing
    "replace": "s.replace('a', 'Z')", "replace_count": "s.replace(',', ';', 1)", "replace_bad": "s.replace(1, 'x')",
    "removeprefix": "s.removeprefix('a')", "removesuffix": "s.removesuffix('c')",
    "translate": "s.translate(str.maketrans('ab', 'xy'))",
    "encode": "s.encode()", "encode_ascii": "s.encode('ascii')",
    # subscripts and operators
    "item0": "s[0]", "item_neg": "s[-1]", "item_far": "s[5]", "item_bad": "s['a']", "slice": "s[1:]",
    "slice_step": "s[::-1]", "slice_mid": "s[1:3]",
    "add": "s + 'x'", "radd": "'x' + s", "add_bad": "s + 1", "mul": "s * 2", "mul_bad": "s * 'a'",
    "eq": "s == 'a'", "lt": "s < 'b'", "lt_bad": "s < 1", "mod_none": "s % ()",
    # builtins
    "len": "len(s)", "int": "int(s)", "float": "float(s)", "ord": "ord(s)", "repr": "repr(s)", "str": "str(s)",
    "bool": "bool(s)", "sorted": "sorted(s)", "list": "list(s)", "set_len": "len(set(s))", "max": "max(s)",
    "hash_eq": "hash(s) == hash(s + '')",
    "find_range": "s.find('a', 1, 3)", "count_range": "s.count('a', 0, -1)", "index_neg": "s.index('a', -2)",
    "zfill_sign": "s.zfill(5)", "format_map": "'{x}'.format_map({'x': s})", "encode_bad": "s.encode('nope')",
    "decode_rt": "s.encode('utf-8').decode('ascii')", "int_base": "int(s, 16)", "int_base0": "int(s, 0)",
    "complex": "complex(s)", "chars": "[c for c in s]", "enumerate": "list(enumerate(s))",
    "rev_join": "''.join(reversed(s))", "min_bad": "min(s, 1)", "splitlines_lit": "s.splitlines()[0]",
    "split_join": "','.join(s.split(','))", "title_split": "[w.title() for w in s.split()]",
    "upper_lower": "s.upper().lower()", "strip_split": "s.strip().split(' ')", "str_mul_neg": "s * -1",
    "contains_empty": "'' in s", "isdigit_int": "int(s) if s.isdigit() else -1", "ljust_bad": "s.ljust('4')",
    # formatting s
    "fstr": "f'<{s}>'", "fstr_r": "f'{s!r}'", "fstr_w": "f'{s:>5}'", "fstr_fill": "f'{s:*^6}'",
    "fstr_trunc": "f'{s:.2}'", "fstr_d": "f'{s:d}'", "format_m": "'[{}]'.format(s)", "format_idx": "'{0}{0}'.format(s)",
    "format_kw": "'{a}'.format(a=s)", "format_missing": "'{1}'.format(s)", "format_w": "'{:<4}|'.format(s)",
    "pct_s": "'%s!' % s", "pct_r": "'%r' % (s,)", "pct_d": "'%d' % (s,)", "pct_w": "'%-4s|' % (s,)",
    "pct_few": "'%s %s' % (s,)", "pct_dict": "'%(k)s' % {'k': s}", "format_fn": "format(s, '^5')",
}

# numbers formatted: (value, specs)
NUMBERS = {
    "int": "7", "neg": "-12", "zero": "0", "big": "1234567", "float": "2.5", "neg_float": "-0.125",
    "bool": "True", "none": "None",
    "joined": "3 if c1 else -4",
    "unknown": "int(os.environ.get('RI{N}', '15'))",
    "unknown_float": "float(os.environ.get('RF{N}', '1.75'))",
}
FORMATS = {
    "fstr": "f'{x}'", "fstr_r": "f'{x!r}'", "fstr_d": "f'{x:d}'", "fstr_w": "f'{x:5}'", "fstr_zero": "f'{x:05}'",
    "fstr_x": "f'{x:x}'", "fstr_X": "f'{x:#X}'", "fstr_b": "f'{x:b}'", "fstr_o": "f'{x:o}'", "fstr_c": "f'{x:,}'",
    "fstr_us": "f'{x:_}'", "fstr_sign": "f'{x:+}'", "fstr_space": "f'{x: }'", "fstr_f": "f'{x:.2f}'",
    "fstr_e": "f'{x:.1e}'", "fstr_g": "f'{x:g}'", "fstr_pct": "f'{x:.0%}'", "fstr_left": "f'{x:<4}|'",
    "fstr_center": "f'{x:^6}'", "fstr_eq": "f'{x:=+6}'", "fstr_s": "f'{x:s}'", "fstr_bad": "f'{x:q}'",
    "fstr_n": "f'{x:n}'", "fstr_dot": "f'{x:.3}'",
    "format_fn": "format(x, '>4')", "format_m": "'{:.1f}'.format(x)", "format_d": "'{:d}'.format(x)",
    "pct_d": "'%d' % (x,)", "pct_i": "'%i' % x", "pct_5d": "'%5d' % (x,)", "pct_02d": "'%02d' % (x,)",
    "pct_f": "'%f' % (x,)", "pct_2f": "'%.2f' % (x,)", "pct_e": "'%e' % (x,)", "pct_g": "'%g' % (x,)",
    "pct_x": "'%x' % (x,)", "pct_o": "'%o' % (x,)", "pct_s": "'%s' % (x,)", "pct_r": "'%r' % (x,)",
    "pct_plus": "'%+d' % (x,)", "pct_left": "'%-5d|' % (x,)", "pct_c": "'%c' % (x,)", "pct_star": "'%*d' % (4, x)",
    "str": "str(x)", "repr": "repr(x)", "add_str": "'n=' + str(x)", "concat_bad": "'n=' + x",
    "fstr_cw": "f'{x:>8,.2f}'", "fstr_hash": "f'{x:#o}'", "fstr_Ec": "f'{x:E}'", "fstr_G": "f'{x:.3G}'",
    "fstr_nested": "f'{x:{4}}'", "fstr_dbg": "f'{x=}'", "format_comma": "'{:,}'.format(x)", "format_bad": "'{:z}'.format(x)",
    "pct_tuple": "'%s' % ((x, 1),)", "pct_many": "'%d%%' % x", "pct_space": "'% d' % x", "pct_hash_x": "'%#x' % x",
    "pct_prec_s": "'%.1s' % (x,)", "int_of": "int(x)", "round": "round(x)", "round1": "round(x, 1)",
    "divmod": "divmod(x, 3)", "floordiv": "x // 2", "mod": "x % 3", "truediv": "x / 4", "pow": "x ** 2",
    "neg": "-x", "abs": "abs(x)", "hex": "hex(x)", "bin": "bin(x)", "chr": "chr(x)", "bool": "bool(x)",
}

TAIL = [
    "except TypeError:",
    "    r = 'TypeError!'",
    "except ValueError:",
    "    r = 'ValueError!'",
    "except IndexError:",
    "    r = 'IndexError!'",
    "except KeyError:",
    "    r = 'KeyError!'",
    "except Exception:",
    "    r = 'Exception!'",
    "if r is None or isinstance(r, (bool, int, float, str, bytes)):",
    "    O = r",
    "else:",
    "    O = repr(r)",
]


def program(name: str, value: str, expr: str, desc: str) -> Prog:
    lines = ["def p{N}():", "    global O", f"    {name} = {value}", "    try:", f"        r = {expr}"]
    lines += ["    " + ln for ln in TAIL]
    return Prog(lines, desc=desc)


def programs():
    for (rk, rv), (ok, ov) in itertools.product(RECEIVERS.items(), OPS.items()):
        yield program("s", rv, ov, f"str {rk} {ok}")
    for (nk, nv), (fk, fv) in itertools.product(NUMBERS.items(), FORMATS.items()):
        yield program("x", nv, fv, f"num {nk} {fk}")


if __name__ == "__main__":
    cli(programs, NAMES)
