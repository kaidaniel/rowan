"""Hypothesis: a ChainMap reads through its maps in order but writes and deletes touch only the
first map: `cm[k] = v` for a key that lives only in a later map adds it to the first (the later
map keeps its value), `del cm[k]` for a key that is only in a later map raises KeyError, and
new_child() puts a fresh dict in front. Exact key sets of every underlying dict; with overrides
from the environment the read of `host` succeeds (the defaults have it) but its value is open."""
from collections import ChainMap
import json
import os


def layered(defaults, overrides):
    return ChainMap(overrides, defaults)


def apply(cm, changes):
    for k, v in changes.items():
        cm[k] = v
    return cm


def drop(cm, key):
    try:
        del cm[key]
        return True
    except KeyError:
        return False


if __name__ == "__main__":
    defaults = {"host": "h0", "port": 80}
    over = {"port": 8080}
    cm = layered(defaults, over)
    p = cm["port"]                               #@value p: exactly={8080}
    apply(cm, {"host": "h1", "debug": True})
    #@row over: present={port,host,debug} definite={port,host,debug} closed
    #@row defaults: present={host,port} definite={host,port} closed
    dh = defaults["host"]                        #@value dh: exactly={"h0"}
    ch = cm["host"]                              #@value ch: exactly={"h1"}
    d1 = drop(cm, "host")                        #@value d1: exactly={True}
    ch2 = cm["host"]                             #@value ch2: exactly={"h0"}
    d2 = drop(cm, "host")                        #@value d2: exactly={False}
    child = cm.new_child({"x": 1})
    child["port"] = 1
    front = child.maps[0]
    #@row front: present={x,port} definite={x,port} closed
    cp = cm["port"]                              #@value cp: exactly={8080}
    env_over = json.loads(os.environ.get("S121_O", '{"port": 9}'))
    cm2 = layered({"host": "h"}, env_over)
    h2 = cm2["host"]                             #@ ok host
    flat = dict(cm2)
    #@row flat: present={host} definite={host} open
