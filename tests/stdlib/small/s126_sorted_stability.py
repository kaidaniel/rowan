"""Hypothesis: sorted(key=…, reverse=True) keeps records with equal keys in their original order
(reverse does not flip ties), unlike sorted(…)[::-1]; a tuple key with a negated field sorts one
field descending and the other ascending; max / min return the FIRST record with the extreme
key. Records come from a builder helper with literal keys; records from the environment must
have `score` and `name`."""
import json
import os


def rec(name, score, age):
    return {"name": name, "score": score, "age": age}


def people():
    return [rec("a", 2, 30), rec("b", 3, 20), rec("c", 2, 25), rec("d", 3, 40)]


def names(rs):
    return "".join(r["name"] for r in rs)


def by_score_desc(rs):
    return sorted(rs, key=lambda r: r["score"], reverse=True)


def flipped(rs):
    return sorted(rs, key=lambda r: r["score"])[::-1]


def ranked(rs):
    return sorted(rs, key=lambda r: (-r["score"], r["age"]))


if __name__ == "__main__":
    n1 = names(by_score_desc(people()))          #@value n1: exactly={"bdac"}
    n2 = names(flipped(people()))                #@value n2: exactly={"dbca"}
    n3 = names(ranked(people()))                 #@value n3: exactly={"bdca"}
    top = by_score_desc(people())[0]
    #@row top: present={name,score,age} definite={name,score,age} closed
    tn = top["name"]                             #@value tn: exactly={"b"}
    mx = max(people(), key=lambda r: r["score"])["name"]   #@value mx: exactly={"b"}
    mn = min(people(), key=lambda r: r["score"])["name"]   #@value mn: exactly={"a"}
    rs = json.loads(os.environ.get("S126_R", '[{"name": "x", "score": 1}, {"name": "y", "score": 1}]'))
    #@schema json.loads@L43['*']: always={score,name} only={score,name}
    en = names(by_score_desc(rs))
    ln = len(en) >= 0                            #@value ln: exactly={True}
