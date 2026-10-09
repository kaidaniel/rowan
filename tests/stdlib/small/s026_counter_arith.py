# Counter arithmetic: + drops non-positive counts, - keeps only positive, & min, | max; update / subtract
from collections import Counter


def merge_counts(*cs):
    total = Counter()
    for c in cs:
        total.update(c)
    return total


def arith():
    c1 = Counter(a=3, b=1)
    c2 = Counter(a=1, b=2, c=1)
    s = c1 + c2                                  #@row s: present={a,b,c} definite={a,b,c} closed
    d = c1 - c2                                  #@row d: present={a} definite={a} absent={b,c} closed
    i = c1 & c2                                  #@row i: present={a,b} definite={a,b} absent={c} closed
    u = c1 | c2                                  #@row u: present={a,b,c} definite={a,b,c} closed
    c1.subtract(c2)
    _ = 0                                        #@row c1: present={a,b,c} definite={a,b,c} closed
    neg = c1["b"]                                #@value neg: exactly={-1}
    pos = +c1                                    #@row pos: present={a} definite={a} absent={b,c} closed
    return s


if __name__ == "__main__":
    m = merge_counts(Counter("ab"), {"b": 2, "z": 1})  #@row m: present={a,b,z} definite={a,b,z} closed
    nb = m["b"]                                  #@value nb: exactly={3}
    e = merge_counts()                           #@row e: present={} closed
    arith()
