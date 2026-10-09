# heapq: (priority, seq, record) tuples pop in priority order; nlargest / nsmallest with key= read keys

import heapq


def schedule(tasks):
    heap = []
    for i, t in enumerate(tasks):
        heapq.heappush(heap, (t["prio"], i, t))
    order = []
    while heap:
        _, _, t = heapq.heappop(heap)
        order.append(t["name"])                   #@ ok name
    return order
#@schema param schedule.tasks['*']: always={prio,name} only={prio,name}


def top_spenders(customers, n):
    return heapq.nlargest(n, customers, key=lambda c: c["spent"])


o = schedule([{"name": "b", "prio": 2}, {"name": "a", "prio": 1}, {"name": "c", "prio": 3, "owner": "x"}])
o0 = o[0]                                        #@value o0: exactly={"a"}
ne = len(schedule([]))                           #@value ne: exactly={0}
top = top_spenders([{"id": 1, "spent": 5}, {"id": 2, "spent": 9}, {"id": 3, "spent": 7}], 2)
t0 = top[0]                                      #@row t0: present={id,spent} definite={id,spent} closed
tid = t0["id"]                                   #@value tid: exactly={2}
nt = len(top)                                    #@value nt: exactly={2}
low = heapq.nsmallest(1, [{"w": 3, "k": "x"}, {"w": 1, "k": "y"}], key=lambda d: d["w"])[0]
lk = low["k"]                                    #@value lk: exactly={"y"}
h = [5, 1, 4]
heapq.heapify(h)
m = h[0]                                         #@value m: exactly={1}
