"""A fake installed package for z716_library_share.py whose import alone needs more work than
the library's share under `#@limits budget=64` (64 entry allowances of 4 budgets: 16,384
statements): its module code counts to 16**4 = 65,536 in loops rowan unrolls."""

ROWS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
COUNT = 0
for a in ROWS:
    for b in ROWS:
        for c in ROWS:
            for d in ROWS:
                COUNT += 1
NAME = "share"
