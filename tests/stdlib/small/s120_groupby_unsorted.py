"""Hypothesis: itertools.groupby groups only consecutive records with equal keys: on unsorted
records a key met twice gives two groups, and a dict built from the groups keeps the LAST
group of that key (the first is overwritten); sorting first through the same key helper merges
them (stable: original order inside a group). Exact on literal records; teams from the
environment give an open row, each record needs `team` and `name`."""
import itertools
import json
import os


def key_of(rec):
    return rec["team"]


def runs(records):
    return [(k, len(list(g))) for k, g in itertools.groupby(records, key=key_of)]


def by_team(records, presort):
    if presort:
        records = sorted(records, key=key_of)
    out = {}
    for k, g in itertools.groupby(records, key=key_of):
        out[k] = [r["name"] for r in g]
    return out


if __name__ == "__main__":
    R = [{"team": "a", "name": "x"}, {"team": "b", "name": "y"}, {"team": "a", "name": "z"}]
    r = runs(R)
    nr = len(r)                                  #@value nr: exactly={3}
    r2 = r[2]                                    #@value r2: exactly={("a", 1)}
    e0 = len(runs([]))                           #@value e0: exactly={0}
    u = by_team(R, False)
    #@row u: present={a,b} definite={a,b} closed
    nua = len(u["a"])                            #@value nua: exactly={1}
    ua = u["a"][0]                               #@value ua: exactly={"z"}
    s = by_team(R, True)
    #@row s: present={a,b} definite={a,b} closed
    sa = "".join(s["a"])                         #@value sa: exactly={"xz"}
    sb = s["b"][0]                               #@value sb: exactly={"y"}
    E = json.loads(os.environ.get("S120_R", '[{"team": "q", "name": "n1"}, {"team": "q", "name": "n2"}]'))
    #@schema json.loads@L42['*']: always={team,name} only={team,name}
    e = by_team(E, True)
    #@row e: present={} definite={} open
