# A for loop over an iterator its body takes items from too (`next(words)`): Python's loop
# takes each next item where the iterator stands then, so the items the body took are
# skipped. Over iter() of a list, a generator, and with a break; the iterator after the loop.
def parse(s):
    words = iter(s.split())
    out = []
    for w in words:
        if w == "on":
            w2 = next(words, "")
            out.append("bg:" + w2)
        else:
            out.append("fg:" + w)
    return out


r = parse("red on blue")
n = len(r)
#@value n: exactly={2}
second = r[1]
#@value second: exactly={"bg:blue"}


def gen():
    yield 1
    yield 2
    yield 3
    yield 4


g = gen()
total = 0
for x in g:
    total += x
    next(g, None)
t4 = total
#@value t4: exactly={4}
it = iter([5, 6, 7, 8])
seen = []
for y in it:
    seen.append(y)
    if y == 7:
        break
    next(it)
rest = list(it)
nr = len(rest)
#@value nr: exactly={1}
last = rest[0]
#@value last: exactly={8}
ns = len(seen)
#@value ns: exactly={2}
