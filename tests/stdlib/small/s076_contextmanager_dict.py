# contextlib.contextmanager yielding dicts: the with-target is the yielded dict; code after the yield changes it

from contextlib import contextmanager

LOG = []


@contextmanager
def transaction(base):
    tx = {"id": base["id"], "ops": []}
    yield tx
    tx["closed"] = True
    LOG.append(tx)
#@schema param transaction.base: always={id} only={id}


@contextmanager
def tolerant(report):
    try:
        yield report
    except KeyError as e:
        report["error"] = e.args[0]


def apply(base, ops):
    with transaction(base) as tx:
        n0 = len(tx)                              #@row tx: present={id,ops} definite={id,ops} closed
        for op in ops:
            tx["ops"].append(op["name"])          #@ ok name
        if ops:
            tx["last"] = ops[-1]["name"]
    done = tx["closed"]                           #@value done: exactly={True}
    #@row tx: present={id,ops,last,closed} definite={id,ops,closed} closed
    return tx


def safe_read(rec):
    rep = {"ok": True}
    with tolerant(rep):
        rep["value"] = rec["value"]               #@ none value
    n = len(rep)                                  #@value n: exactly={2}
    #@row rep: present={ok,value,error} definite={ok} closed
    return rep


r1 = apply({"id": 1, "user": "u"}, [{"name": "ins"}])   #@row r1: present={id,ops,last,closed} definite={id,ops,last,closed} closed
r2 = apply({"id": 2}, [])                                #@row r2: present={id,ops,closed} definite={id,ops,closed} closed
nl = len(LOG)                                            #@value nl: exactly={2}
s1 = safe_read({"value": 5})                             #@row s1: present={ok,value} definite={ok,value} closed
s2 = safe_read({})                                       #@row s2: present={ok,error} definite={ok,error} closed
err = s2["error"]                                        #@value err: exactly={"value"}
