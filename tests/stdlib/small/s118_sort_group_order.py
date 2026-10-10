"""Hypothesis: order-dependent results are exact on literal rows: a tuple sort key with a negated
part, stable ties (sorted, and heapq.nlargest which keeps the first of equal keys), groupby after
a sort (one group per key) versus groupby on unsorted data (a later group overwrites an earlier
one of the same key in a dict comprehension), max(d, key=d.get) returning the first maximal key,
bisect positions on a list kept sorted with insort. Rows from input give unknown names and
team keys (open row) and require team and name of each row."""
import bisect
import heapq
import itertools
import json
import os

ROWS = [
    {"name": "a", "team": "x", "score": 3},
    {"name": "b", "team": "y", "score": 5},
    {"name": "c", "team": "x", "score": 7},
    {"name": "d", "team": "y", "score": 5},
]


def rank(rows):
    return [r["name"] for r in sorted(rows, key=lambda r: (r["team"], -r["score"]))]


def groups(rows):
    rows = sorted(rows, key=lambda r: r["team"])
    return {k: [r["name"] for r in g] for k, g in itertools.groupby(rows, key=lambda r: r["team"])}


def run_lengths(text):
    return {k: len(list(g)) for k, g in itertools.groupby(text)}


def top_word(counts):
    return max(counts, key=counts.get)


def best(rows, n):
    return [r["name"] for r in heapq.nlargest(n, rows, key=lambda r: r["score"])]


def insert_all(xs):
    out, pos = [], []
    for x in xs:
        pos.append(bisect.bisect_left(out, x))
        bisect.insort(out, x)
    return out, pos


if __name__ == "__main__":
    order = tuple(rank(ROWS))                    #@value order: exactly={("c","a","b","d")}
    g = groups(ROWS)
    #@row g: present={x,y} definite={x,y} closed
    gx = tuple(g["x"])                           #@value gx: exactly={("a","c")}
    rl = run_lengths("aabca")
    #@row rl: present={a,b,c} definite={a,b,c} closed
    ra = rl["a"]                                 #@value ra: exactly={1}
    tw = top_word({"p": 2, "q": 5, "r": 5})      #@value tw: exactly={"q"}
    b2 = tuple(best(ROWS, 2))                    #@value b2: exactly={("c","b")}
    out, pos = insert_all([5, 1, 4, 1, 9])
    so = tuple(out)                              #@value so: exactly={(1,1,4,5,9)}
    sp = tuple(pos)                              #@value sp: exactly={(0,0,1,0,4)}
    g2 = groups(json.loads(os.environ.get("ROWS", '[{"team": "t", "name": "n"}, {"team": "s", "name": "m", "x": 1}]')))
    #@schema json.loads@L63['*']: always={team,name} only={team,name}
    _ = 0                                        #@row g2: present={} definite={} open
