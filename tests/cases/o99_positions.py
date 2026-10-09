"""Positions through comprehensions with filters, sorted with tuple keys, enumerate over a
sorted list, zip of two known lists, and a slice with a computed bound.

Expectations were written from the program text before running the engine."""

TASKS = [
    {"id": "t1", "pri": 2, "kind": "pick", "extra": 1},
    {"id": "t2", "pri": 0, "kind": "ship"},
    {"id": "t3", "pri": 1, "kind": "pick", "note": "x"},
    {"id": "t4", "pri": 0, "kind": "receive", "late": True},
]


def ranked(tasks):
    return sorted(tasks, key=lambda t: (t["pri"], t["id"]))


picks = [t for t in TASKS if t["kind"] == "pick"]
first_pick = picks[0]                     #@row first_pick: present={id,pri,kind,extra} definite={id,pri,kind,extra} closed
second_pick = picks[1]                    #@row second_pick: present={id,pri,kind,note} definite={id,pri,kind,note} closed
order = ranked(TASKS)
# pri 0: t2, t4 (by id); then t3 (pri 1); then t1 (pri 2).
top = order[0]                            #@row top: present={id,pri,kind} definite={id,pri,kind} closed
second = order[1]                         #@row second: present={id,pri,kind,late} definite={id,pri,kind,late} closed
last = order[3]                           #@row last: present={id,pri,kind,extra} definite={id,pri,kind,extra} closed
numbered = [{"rank": i + 1, "id": t["id"]} for i, t in enumerate(order)]
n0 = numbered[0]                          #@row n0: present={rank,id} definite={rank,id} closed
by_rank = {t["id"]: i for i, t in enumerate(order)}  #@row by_rank: present={t1,t2,t3,t4} definite={t1,t2,t3,t4} closed
names = ["ann", "bob"]
tiers = ["gold", "basic"]
pairs = {n: {"tier": tr} for n, tr in zip(names, tiers)}  #@row pairs: present={ann,bob} definite={ann,bob} closed
limit = len(names)
head = order[:limit]
head1 = head[1]                           #@row head1: present={id,pri,kind,late} definite={id,pri,kind,late} closed
desc = sorted(TASKS, key=lambda t: t["pri"], reverse=True)
hi = desc[0]                              #@row hi: present={id,pri,kind,extra} definite={id,pri,kind,extra} closed
top["late"]                               #@ missing late
second["late"]                            #@ ok late
