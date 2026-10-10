"""Hypothesis: a generator keeps its position across consumers: `next(g)` after a `for` that
broke out resumes after the item the loop saw last; `islice(g, 3)` pulls exactly three items;
`zip(g, "ab")` pulls one extra item from g before noticing the short side is exhausted, while
`zip("ab", g)` does not; a generator that `return`s at a blank line yields only the pairs before
it. Values and rows are exact on literal inputs; pairs from input give an open dict."""
import itertools
import json
import os


def counter(start, log):
    n = start
    while True:
        log.append(n)
        yield n
        n += 1


def first_after_break(g, stop):
    for x in g:
        if x >= stop:
            break
    return next(g)


def until_blank(lines):
    for ln in lines:
        if not ln.strip():
            return
        k, _, v = ln.partition("=")
        yield k.strip(), v.strip()


def gen_records(rows):
    for r in rows:
        if r.get("skip"):
            continue
        yield {"id": r["id"], "n": len(r)}
    yield {"id": "end", "last": True}


if __name__ == "__main__":
    log = []
    v = first_after_break(counter(10, log), 12)  #@value v: exactly={13}
    nlog = len(log)                              #@value nlog: exactly={4}
    log2 = []
    head = list(itertools.islice(counter(0, log2), 3))
    h2 = head[2]                                 #@value h2: exactly={2}
    nlog2 = len(log2)                            #@value nlog2: exactly={3}
    g = counter(0, [])
    pairs = list(zip(g, "ab"))
    nxt = next(g)                                #@value nxt: exactly={3}
    g2 = counter(0, [])
    pairs2 = list(zip("ab", g2))
    nxt2 = next(g2)                              #@value nxt2: exactly={2}
    np2 = len(pairs2)                            #@value np2: exactly={2}
    cfg = dict(until_blank(["a = 1", "b=2", "  ", "c=3"]))
    #@row cfg: present={a,b} definite={a,b} absent={c} closed
    cb = cfg["b"]                                #@value cb: exactly={"2"}
    recs = gen_records([{"id": 1, "x": 2}, {"id": 2, "skip": True}, {"id": 3}])
    r0 = next(recs)
    #@row r0: present={id,n} definite={id,n} closed
    n0 = r0["n"]                                 #@value n0: exactly={2}
    rest = list(recs)
    tail = rest[-1]
    #@row tail: present={id,last} definite={id,last} closed
    mid = rest[0]["id"]                          #@value mid: exactly={3}
    cfg2 = dict(until_blank(json.loads(os.environ.get("LINES", '["x=1", "", "y=2"]'))))
    #@row cfg2: present={} definite={} open
