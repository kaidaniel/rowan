# itertools.product / permutations / combinations: counts and tuples from literal inputs; dicts built per tuple

from itertools import combinations, permutations, product


def grid(sizes, colors):
    variants = [{"size": s, "color": c} for s, c in product(sizes, colors)]
    n = len(variants)                              #@value n: exactly={6,0}
    return variants


def pairs(people):
    out = []
    for a, b in combinations(people, 2):
        out.append({"pair": (a["name"], b["name"])})   #@ ok name
    return out
#@schema param pairs.people['*']: always={name} only={name}


g = grid(["S", "M"], ["red", "blue", "green"])
v0 = g[0]                                          #@row v0: present={size,color} definite={size,color} closed
c0 = v0["color"]                                   #@value c0: exactly={"red"}
s4 = g[4]["size"]                                  #@value s4: exactly={"M"}
grid([], ["x"])
nperm = len(list(permutations("abc")))             #@value nperm: exactly={6}
ncomb = len(list(combinations(range(4), 3)))       #@value ncomb: exactly={4}
npr = len(list(product([0, 1], repeat=2)))         #@value npr: exactly={4}
fp = "".join(next(permutations("abc")))            #@value fp: exactly={"abc"}
lp = len(pairs([{"name": "a"}, {"name": "b"}, {"name": "c", "age": 3}]))   #@value lp: exactly={3}
l1 = len(pairs([{"name": "x"}]))                   #@value l1: exactly={0}
