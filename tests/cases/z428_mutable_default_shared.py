"""Hypothesis: a mutable default argument is one dict shared by every call that omits it: keys
stored by earlier calls are there in later ones, and the returned objects are the same object.
A call that passes its own dict does not touch the default; the `None` idiom gives a fresh dict.
Precise: after collect("x"), collect("y"), collect("x") the default holds {x: 2, y: 1}."""
import json
import os


def collect(k, acc={}):
    acc[k] = acc.get(k, 0) + 1
    return acc


def fresh(k, acc=None):
    if acc is None:
        acc = {}
    acc[k] = 1
    return acc


if __name__ == "__main__":
    a = collect("x")
    #@row a: present={x} definite={x} closed
    b = collect("y")
    #@row b: present={x,y} definite={x,y} closed
    same = a is b                                #@value same: exactly={True}
    _ = 0                                        #@row a: present={x,y} definite={x,y} closed
    c = collect("x")
    cx = c["x"]                                  #@value cx: exactly={2}
    other = collect("z", {})
    #@row other: present={z} definite={z} absent={x,y} closed
    _ = 0                                        #@row a: present={x,y} definite={x,y} absent={z} closed
    f1 = fresh("p")
    f2 = fresh("q")
    _ = 0                                        #@row f1: present={p} definite={p} absent={q} closed
    k = json.loads(os.environ.get("Z428_K", '"y"'))
    m = len(collect(k))                          #@value m: includes={2,3}
    yy = a["y"]                                  #@value yy: includes={1,2}
