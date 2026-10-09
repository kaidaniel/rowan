# itertools.batched (3.12+): tuples of n records, the last one shorter

from itertools import batched


def chunk_ids(rows, n):
    out = []
    for batch in batched(rows, n):
        out.append({"ids": [r["id"] for r in batch], "size": len(batch)})
    return out
#@schema param chunk_ids.rows['*']: always={id} only={id}


c = chunk_ids([{"id": 1}, {"id": 2}, {"id": 3, "x": 0}], 2)
nc = len(c)                                       #@value nc: exactly={2}
last = c[-1]                                      #@row last: present={ids,size} definite={ids,size} closed
sz = last["size"]                                 #@value sz: exactly={1}
e = chunk_ids([], 3)
ne = len(e)                                       #@value ne: exactly={0}
bs = list(batched("abcdefg", 3))
b2 = bs[2]                                        #@value b2: exactly={("g",)}
nb = len(bs)                                      #@value nb: exactly={3}
pair = next(batched([{"a": 1}, {"b": 2}], 2))
p1 = pair[1]                                      #@row p1: present={b} definite={b} closed
