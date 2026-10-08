"""Ranking records: max with a key lambda, enumerate over a sorted list (the order is open but
the count is known), zip with literal medal names.

Expectations were written from the program text before running the engine."""

PLAYERS = [{"name": "a", "score": 10}, {"name": "b", "score": 30}, {"name": "c", "score": 20}]

best = max(PLAYERS, key=lambda p: p["score"])  #@row best: present={name,score} definite={name,score} closed
ranked = sorted(PLAYERS, key=lambda p: p["score"], reverse=True)
ranking = {}
for i, p in enumerate(ranked):
    ranking[i] = p["name"]
_ = 0                                     #@row ranking: present={0,1,2} definite={0,1,2} closed
by_rank = {p["name"]: i for i, p in enumerate(PLAYERS)}  #@row by_rank: present={a,b,c} definite={a,b,c} closed
medals = dict(zip(["gold", "silver", "bronze"], ranked))  #@row medals: present={gold,silver,bronze} definite={gold,silver,bronze} closed
gold = medals["gold"]                     #@row gold: present={name,score} definite={name,score} closed
podium = [{"place": i + 1, **p} for i, p in enumerate(ranked)]
p0 = podium[0]                            #@row p0: present={place,name,score} definite={place,name,score} closed
ranking[3]                                #@ missing 3
medals["gold"]["score"]                   #@ ok score
