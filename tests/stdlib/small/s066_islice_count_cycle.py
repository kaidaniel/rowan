# islice / count / cycle + zip / repeat: positions from literal inputs, records tagged with counters

from itertools import count, cycle, islice, repeat


def number(rows, start=1):
    out = []
    for i, r in zip(count(start), rows):
        out.append({"n": i, **r})
    last = out[-1] if out else {}
    #@row last: present={n,b} closed
    return out


def assign(workers, tasks):
    plan = [{"task": t, "worker": w} for t, w in zip(tasks, cycle(workers))]
    return plan


nums = number([{"a": 1}, {"b": 2}], start=10)
r1 = nums[1]                                      #@row r1: present={n,b} definite={n,b} closed
n1 = r1["n"]                                      #@value n1: exactly={11}
empty = number([])
p = assign(["w1", "w2"], ["t1", "t2", "t3"])
w3 = p[2]["worker"]                               #@value w3: exactly={"w1"}
np_ = len(p)                                      #@value np_: exactly={3}
first3 = list(islice(count(5, 5), 3))
s = sum(first3)                                   #@value s: exactly={30}
head = list(islice([{"k": 1}, {"k": 2}, {"j": 3}], 1, None))
h1 = head[1]                                      #@row h1: present={j} definite={j} closed
z = list(repeat({"x": 0}, 2))
same = z[0] is z[1]                               #@value same: exactly={True}
cyc = "".join(islice(cycle("ab"), 5))             #@value cyc: exactly={"ababa"}
