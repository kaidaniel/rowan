# itertools.groupby with key=: each group iterates over the caller's records sharing the key

from itertools import groupby
from operator import itemgetter


def per_team(players):
    players = sorted(players, key=lambda p: p["team"])
    out = {}
    for team, members in groupby(players, key=lambda p: p["team"]):
        ms = list(members)
        first = ms[0]
        #@row first: present={team,name,score} definite={team,name} closed
        out[team] = [m["name"] for m in ms]          #@ ok name
    return out
#@schema param per_team.players['*']: always={team,name} only={team,name}


def best_by_team(players):
    res = {}
    for team, grp in groupby(sorted(players, key=itemgetter("team")), key=itemgetter("team")):
        res[team] = max(grp, key=lambda p: p.get("score", 0))
    return res


t = per_team([{"team": "b", "name": "x"}, {"team": "a", "name": "y", "score": 3}, {"team": "b", "name": "z"}])
#@row t: present={a,b} definite={a,b} closed
nb = len(t["b"])                                   #@value nb: exactly={2}
e = per_team([])                                   #@row e: present={} closed
r = best_by_team([{"team": "r", "name": "p", "score": 2}, {"team": "r", "name": "q", "score": 5}])
#@row r: present={r} definite={r} closed
w = r["r"]["name"]                                 #@value w: exactly={"q"}
ks = [k for k, _ in groupby("aabbbc")]
nk = len(ks)                                       #@value nk: exactly={3}
try:
    t["c"]                                         #@ missing c
except KeyError:
    pass
