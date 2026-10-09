# defaultdict: a read of an absent key never fails and adds the key; factories list / int / lambda
from collections import defaultdict


def group(rows):
    g = defaultdict(list)
    for r in rows:
        g[r["dept"]].append(r["name"])
    return g
#@schema param group.rows['*']: always={dept,name} only={dept,name}


def count_words(text):
    counts = defaultdict(int)
    for w in text.split():
        counts[w] += 1
    return counts


if __name__ == "__main__":
    g = group([{"dept": "eng", "name": "a"}, {"dept": "ops", "name": "b"}, {"dept": "eng", "name": "c"}])
    #@row g: present={eng,ops} definite={eng,ops} closed
    n = len(g["eng"])                            #@value n: exactly={2}
    x = g["hr"]                                  #@ ok hr
    #@row g: present={eng,ops,hr} definite={eng,ops,hr} closed
    y = g.get("qa")                              #@value y: exactly={None}
    q = "qa" in g                                #@value q: exactly={False}
    w = count_words("to be or not to be")        #@row w: present={to,be,or,not} definite={to,be,or,not} closed
    tb = w["to"]                                 #@value tb: exactly={2}
    lam = defaultdict(lambda: "n/a", {"k": "v"})
    v = lam["z"]                                 #@value v: exactly={"n/a"}
    _ = 0                                        #@row lam: present={k,z} definite={k,z} closed
