# executor.map(f, items): f runs on each item in worker threads; results come back in input order

from concurrent.futures import ThreadPoolExecutor


def score(rec):
    base = rec["hits"] * 2
    if "bonus" in rec:
        base += rec["bonus"]                      #@ ok bonus
    return {"user": rec["user"], "score": base}
#@schema param score.rec: always={hits,user} only={hits,user}


def score_all(recs):
    with ThreadPoolExecutor(max_workers=3) as ex:
        out = list(ex.map(score, recs))
    return out


def pair_sum(a, b):
    return a + b


res = score_all([{"user": "a", "hits": 3}, {"user": "b", "hits": 1, "bonus": 10}])
s1 = res[1]["score"]                            #@value s1: exactly={12}
r0 = res[0]                                     #@row r0: present={user,score} definite={user,score} closed
u0 = r0["user"]                                 #@value u0: exactly={"a"}
e = score_all([])
ne = len(e)                                     #@value ne: exactly={0}
with ThreadPoolExecutor() as ex:
    sums = list(ex.map(pair_sum, [1, 2], [10, 20]))
    lens = list(ex.map(len, ["ab", "cde"]))
s = sums[1]                                     #@value s: exactly={22}
l1 = lens[1]                                    #@value l1: exactly={3}
try:
    r0["bonus"]                                 #@ missing bonus
except KeyError:
    pass
