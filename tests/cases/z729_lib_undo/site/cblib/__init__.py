"""A fake installed package for z729_lib_callback_undo.py: `run_all` calls the program's
callbacks, then does more work than the library's share under `#@limits budget=64` (16,384
statements): 16**4 = 65,536 steps in loops rowan unrolls."""

ROWS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]


def run_all(cbs, d):
    for cb in cbs:
        cb(d)
    count = 0
    for a in ROWS:
        for b in ROWS:
            for c in ROWS:
                for e in ROWS:
                    count += 1
    d["count"] = count
    return count


def first(cbs, d):
    cbs[0](d)
    return "first"
