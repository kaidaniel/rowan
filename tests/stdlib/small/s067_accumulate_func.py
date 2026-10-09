# itertools.accumulate with a func merging dicts: each step receives the running dict and the next record

import operator
from itertools import accumulate


def merge(acc, rec):
    out = dict(acc)
    out.update(rec)
    return out


def snapshots(updates):
    states = list(accumulate(updates, merge))
    last = states[-1]
    #@row last: present={a,b,c} definite={a} closed
    return states


def running(nums):
    return list(accumulate(nums, operator.mul, initial=1))


s = snapshots([{"a": 1}, {"b": 2}, {"a": 3, "c": 4}])
n = len(s)                                       #@value n: exactly={3}
mid = s[1]                                       #@row mid: present={a,b} definite={a,b} closed
fin = s[2]                                       #@row fin: present={a,b,c} definite={a,b,c} closed
av = fin["a"]                                    #@value av: exactly={3}
s2 = snapshots([{"a": 0}])
r = running([2, 3, 4])
lastv = r[-1]                                    #@value lastv: exactly={24}
nr = len(r)                                      #@value nr: exactly={4}
tot = list(accumulate([1, 2, 3]))[-1]            #@value tot: exactly={6}
try:
    mid["c"]                                     #@ missing c
except KeyError:
    pass
