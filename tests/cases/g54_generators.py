"""Generators, generator expressions, next(), yield from, consumed by loops and list()."""


def gen_rows(n):
    for i in range(n):
        yield {"i": i, "even": i % 2 == 0}


for r in gen_rows(3):
    r["i"]                               #@ ok i
    r["odd"]                             #@ missing odd

rows = list(gen_rows(2))
rows[0]["even"]                          #@ ok even
first = next(gen_rows(1))
first["i"]                               #@ ok i
first["j"]                               #@ missing j


def chain():
    yield {"a": 1}
    yield from gen_rows(1)
    yield {"a": 2, "b": 3}


for c in chain():
    c["a"]                               #@ possibly a
    c["i"]                               #@ possibly i
    c["zz"]                              #@ missing zz

ge = ({"g": x} for x in range(3))
for g in ge:
    g["g"]                               #@ ok g
    g["h"]                               #@ missing h
sq = [v["g"] for v in ({"g": x} for x in range(2))]      #@ ok g


def pipeline(items):
    stage1 = ({**it, "s1": True} for it in items)
    stage2 = ({**it, "s2": True} for it in stage1)
    return list(stage2)


out = pipeline([{"base": 1}])
out[0]["s1"]                             #@ ok s1
out[0]["s2"]                             #@ ok s2
out[0]["base"]                           #@ ok base
out[0]["s3"]                             #@ missing s3
