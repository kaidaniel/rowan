"""A fake installed package for n99_lib_budgets.py: under `#@limits budget=64`, `expensive`
and `tag_all` need more work than one entry into an installed package gets, and `_cell` and
`_tag` run past their budgets."""

ROWS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
REGISTRY = {}


def register(name, value):
    REGISTRY[name] = value


def _cell(i, j):
    return i * j


def expensive():
    out = {}
    for i in ROWS:
        for j in ROWS:
            out[i] = _cell(i, j)
    return out


def _tag(d, j):
    d["last"] = j
    return j


def tag_all(d):
    for i in ROWS:
        for j in ROWS:
            _tag(d, j)
    return "done"


def cheap(d):
    return {"x": d["k"]}
