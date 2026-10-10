"""Hypothesis: copy.copy of a dict of containers shares the inner dict / list: a mutation of the
inner ones through the copy (in a helper called after the copy) shows in the original, while a
top-level key added to the copy does not; deepcopy shares nothing; {**d} is shallow too, but
rebinding a top-level key of the copy breaks the sharing for that key only. A config from the
environment keeps an open row whose `db` gains the definite key `port`."""
import copy
import json
import os


def base():
    return {"db": {"host": "h"}, "tags": ["a"]}


def tweak(cfg):
    cfg["db"]["port"] = 5432
    cfg["tags"].append("b")
    cfg["extra"] = 1
    return cfg


def snapshot(cfg, deep):
    return copy.deepcopy(cfg) if deep else copy.copy(cfg)


if __name__ == "__main__":
    o = base()
    s = snapshot(o, False)
    tweak(s)
    #@row o: present={db,tags} definite={db,tags} absent={extra} closed
    od = o["db"]
    #@row od: present={host,port} definite={host,port} closed
    nt = len(o["tags"])                          #@value nt: exactly={2}
    sh = s["db"] is o["db"]                      #@value sh: exactly={True}
    o2 = base()
    tweak(snapshot(o2, True))
    od2 = o2["db"]
    #@row od2: present={host} definite={host} absent={port} closed
    nt2 = len(o2["tags"])                        #@value nt2: exactly={1}
    o3 = base()
    s3 = {**o3}
    s3["db"] = {"new": True}
    tweak(s3)
    od3 = o3["db"]
    #@row od3: present={host} definite={host} absent={port} closed
    nt3 = len(o3["tags"])                        #@value nt3: exactly={2}
    o4 = json.loads(os.environ.get("S128_C", '{"db": {"host": "e"}, "tags": [], "x": 0}'))
    #@schema json.loads@L47: always={db,tags} only={db,tags}
    tweak(snapshot(o4, False))
    #@row o4: open
    od4 = o4["db"]
    #@row od4: open
    #@todo row od4: present={port} definite={port} open  (and o4: present={db,tags} definite={db,tags} open — keys read through a shallow copy of an input object, and its shared nested dict, are the original's too)
