# bisect: a grade table over literal breakpoints, insort, bisect_left / bisect_right, key= on records

import bisect

BREAKS = [60, 70, 80, 90]
GRADES = "FDCBA"


def grade(score):
    return GRADES[bisect.bisect(BREAKS, score)]


def grade_rec(rec):
    out = dict(rec)
    out["grade"] = grade(rec["score"])
    return out
#@schema param grade_rec.rec: always={score} only={score}


g1 = grade(95)                                   #@value g1: exactly={"A"}
g2 = grade(60)                                   #@value g2: exactly={"D"}
g3 = grade(12)                                   #@value g3: exactly={"F"}
r = grade_rec({"name": "x", "score": 75})        #@row r: present={name,score,grade} definite={name,score,grade} closed
rg = r["grade"]                                  #@value rg: exactly={"C"}
xs = [1, 3, 7]
bisect.insort(xs, 5)
i = xs.index(5)                                  #@value i: exactly={2}
left = bisect.bisect_left([1, 2, 2, 3], 2)       #@value left: exactly={1}
right = bisect.bisect_right([1, 2, 2, 3], 2)     #@value right: exactly={3}
rows = [{"t": 1}, {"t": 4}, {"t": 9}]
pos = bisect.bisect_left(rows, 5, key=lambda d: d["t"])   #@value pos: exactly={2}
