"""A recursive generator's runs count against its function's statement budget like every other
run; past the budget its calls are unknown code. With 8 statements: `walk` runs for the first
tree (4 + 2 + 2) and once more, `label` 4 times, `tree` (1 statement) 8-9 times, and the
requirement-only runs of the 3 functions add 4: about 21, bounded at 24 (rich made 10k+)."""
#@limits budget=8
#@tabulations <= 24


def label(t):
    return t["v"] * 10


def walk(t):
    yield label(t)
    for c in t["kids"]:
        yield from walk(c)


def tree(v):
    return {"v": v, "kids": [{"v": v + 1, "kids": []}, {"v": v + 2, "kids": []}]}


s0 = sum(walk(tree(1)))
#@value s0: exactly={60}
s1 = sum(walk(tree(2)))
#@value s1: includes={90}
s2 = sum(walk(tree(3)))
#@value s2: includes={120}
s3 = sum(walk(tree(4)))
#@value s3: includes={150}
s4 = sum(walk(tree(5)))
#@value s4: includes={180}
s5 = sum(walk(tree(6)))
#@value s5: includes={210}
s6 = sum(walk(tree(7)))
#@value s6: includes={240}
s7 = sum(walk(tree(8)))
#@value s7: includes={270}
s8 = sum(walk(tree(9)))
#@value s8: includes={300}
s9 = sum(walk(tree(10)))
#@value s9: includes={330}
