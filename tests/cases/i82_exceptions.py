"""try / except / else / finally paths adding keys; a helper that raises when a key is
absent, so whether the except path runs depends on the table passed in.

Expectations were written from the program text before running the engine."""
import sys


class NotFound(Exception):
    pass


def lookup(table, key):
    if key not in table:
        raise NotFound(key)
    return table[key]


def fetch(table, key):
    result = {"key": key}
    try:
        result["value"] = lookup(table, key)
    except NotFound:
        result["error"] = "missing"
    else:
        result["found"] = True
    finally:
        result["done"] = True
    return result


def safe_get(d, k, default=None):
    try:
        return d[k]
    except KeyError:
        return default


TABLE = {"a": 1}
# "a" is in the table: lookup cannot raise, the else branch runs.
hit = fetch(TABLE, "a")                   #@row hit: present={key,value,found,done} definite={key,value,found,done} closed
# "zz" is not: lookup raises, only the except and finally branches run.
miss = fetch(TABLE, "zz")                 #@row miss: present={key,error,done} definite={key,error,done} closed
unk = fetch(TABLE, sys.argv[1])           #@row unk: present={key,value,found,error,done} definite={key,done} closed
hit["error"]                              #@ missing error
miss["value"]                             #@ missing value
unk["found"]                              #@ possibly found
cfg = {"x": 1}
v = safe_get(cfg, "x")
w = safe_get(cfg, "y", 0)
cfg["y"]                                  #@ missing y
