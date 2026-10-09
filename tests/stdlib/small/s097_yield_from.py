# generators and yield from producing dicts: the consumer sees each yielded record; yield from returns a value


def header(meta):
    yield {"type": "header", "title": meta["title"]}
#@schema param header.meta: always={title} only={title}


def body(items):
    for i, it in enumerate(items):
        yield {"type": "item", "pos": i, **it}


def counted(gen):
    n = 0
    for x in gen:
        n += 1
        yield x
    return n


def document(meta, items):
    yield from header(meta)
    n = yield from counted(body(items))
    yield {"type": "footer", "count": n}


def titles(metas):
    out = []
    for m in metas:
        for rec in header(m):
            out.append(rec["title"])              #@ ok title
            #@row rec: present={type,title} definite={type,title} closed
    return out


recs = list(document({"title": "T"}, [{"sku": "a"}, {"sku": "b", "qty": 2}]))
nr = len(recs)                                    #@value nr: exactly={4}
h = recs[0]                                       #@row h: present={type,title} definite={type,title} closed
i1 = recs[2]                                      #@row i1: present={type,pos,sku,qty} definite={type,pos,sku,qty} closed
f = recs[-1]                                      #@row f: present={type,count} definite={type,count} closed
fc = f["count"]                                   #@value fc: exactly={2}
by_sku = dict((r["sku"], r) for r in recs if r["type"] == "item")
#@row by_sku: present={a,b} definite={a,b} closed
ts = titles([{"title": "a", "x": 1}, {"title": "b"}])
t1 = ts[1]                                        #@value t1: exactly={"b"}
