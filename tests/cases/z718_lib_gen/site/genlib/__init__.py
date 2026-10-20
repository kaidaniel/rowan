"""A fake installed package for z718_lib_gen.py: `rows()` makes a generator whose body needs
more work than the library's share under `#@limits budget=64` (64 entry allowances of 4 budgets:
16,384): it counts to 16**4 = 65,536 in loops rowan unrolls, then yields its count. `small()`
yields two items at a few steps' cost."""

ROWS = (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)


def rows():
    count = 0
    for a in ROWS:
        for b in ROWS:
            for c in ROWS:
                for d in ROWS:
                    count += 1
    yield count


def small():
    yield "a"
    yield "b"


def made():
    return rows()
