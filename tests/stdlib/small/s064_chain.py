# itertools.chain / chain.from_iterable over lists of records from several callers

from itertools import chain


def all_rows(left, right):
    rows = list(chain(left, right))
    for r in rows:
        r["id"]                                    #@ ok id
    last = rows[-1]
    #@row last: present={id,b} definite={id} closed
    return rows
#@schema param all_rows.left['*']: always={id} only={id}


def flatten(batches):
    flat = list(chain.from_iterable(batches))
    return flat


rows = all_rows([{"id": 1, "a": 1}], [{"id": 2, "b": 2}])
n = len(rows)                                     #@value n: exactly={2}
r0 = rows[0]                                      #@row r0: present={id,a} definite={id,a} closed
rows2 = all_rows([], [{"id": 3}])
f = flatten([[{"k": 1}], [{"k": 2}, {"k": 3, "x": 0}]])
nf = len(f)                                        #@value nf: exactly={3}
third = f[2]                                       #@row third: present={k,x} definite={k,x} closed
first = f[0]                                       #@row first: present={k} definite={k} closed
s = "".join(chain("ab", ["c", "d"]))               #@value s: exactly={"abcd"}
