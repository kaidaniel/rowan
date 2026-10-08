"""Guards: `in`, `.get`, `setdefault`, try/except, hasattr, early-out patterns."""
import json

cfg = json.load(fh)
if "db" in cfg:
    cfg["db"]                            #@ ok db
    cfg["db"]["host"]                    #@ input host
else:
    cfg["db"]                            #@ missing db
cfg["db"]                                #@ input db
host = cfg.get("host", "localhost")      #@ none host
port = cfg.setdefault("port", 5432)
cfg["port"]                              #@ ok port
if cfg.get("tls"):
    cfg["tls"]                           #@ ok tls
    cfg["tls"]["cert"]                   #@ input cert
if cfg.get("retries") is not None:
    cfg["retries"]                       #@ ok retries
try:
    cfg["user"]                          #@ none user
except KeyError:
    cfg["user"] = "anon"
cfg["user"]                              #@ ok user
try:
    cfg["pw"]                            #@ none pw
except (KeyError, TypeError):
    pass
cfg["pw"]                                #@ input pw
try:
    cfg["token"]                         #@ none token
except Exception:
    pass
try:
    cfg["secret"]                        #@ input secret
except ValueError:
    pass
# .get / .setdefault do not require a key; tls and retries are proven inside their guards
#@schema json.load@L4: always={db,pw,secret}
#@schema json.load@L4['db']: sometimes={host}
#@schema json.load@L4['tls']: sometimes={cert}

closed = {"a": 1}
if "b" in closed:
    closed["b"]                          #@ none b
if "a" in closed:
    closed["a"]                          #@ ok a
if "b" not in closed:
    closed["b"] = 2
closed["b"]                              #@ ok b
if closed.get("c") is None:
    closed["c"] = 3
closed["c"]                              #@ ok c
v = closed["d"] if "d" in closed else 0  #@ none d
w = closed["a"] if "a" in closed else 0  #@ ok a
# `assert "e" in closed` would make the rest of the module dead code (closed row without e)
closed["e"] = 5
closed["e"]                              #@ ok e


class Obj:
    def __init__(self):
        self.x = 1


o = Obj()
# hasattr on a closed instance without y: the body is dead code
if hasattr(o, "y"):
    o.y                                  #@ none y
o.y                                      #@ missing y
try:
    o.z                                  #@ none z
except AttributeError:
    pass
getattr(o, "x")                          #@ ok x
getattr(o, "q", None)                    #@ none q
getattr(o, "q")                          #@ missing q


def need(d, k):
    if k not in d:
        raise KeyError(k)
    return d[k]


need({"a": 1}, "a")                      #@ ok a
need({"a": 1}, "b")                      #@ none b
