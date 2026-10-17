"""A fake installed package for n97_allowance.py: `light` is cheap, `heavy` and `drop_then_work`
need more work than an entry into an installed package gets under `#@limits budget=64`."""

ROUNDS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]


def _step(d, i):
    d["n"] = i
    return i


def light(d):
    d["seen"] = 1
    return {"ok": True}


def heavy(d):
    for i in ROUNDS:
        for j in ROUNDS:
            _step(d, i + j)
    return {"done": d["n"]}


def drop_then_work(d):
    del d["k"]
    heavy(d)
    return d
