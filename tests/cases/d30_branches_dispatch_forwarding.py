REQUIRED = ("host", "port")
DEFAULTS = {"port": 5432, "ssl": False}


def load(user):
    cfg = {**DEFAULTS, **user}
    for k in REQUIRED:
        if k not in cfg:
            raise ValueError(k)
    return cfg


c = load({"host": "h"})              #@row c: present={host,port,ssl} definite={host,port,ssl} closed
c["host"]                            #@ ok host
c["user"]                            #@ missing user
d = load(unknown_cfg)                #@row d: present={port,ssl,host} definite={port,ssl,host} open
d["host"]                            #@ ok host
d["timeout"]                         #@ input timeout


def handle_a(p):
    return {"kind": "a", "value": p["a"]}        #@ input a


def handle_b(p):
    return {"kind": "b", "value": p["b"], "extra": 1}


HANDLERS = {"a": handle_a, "b": handle_b}
r = HANDLERS[kind](payload)          #@row r: present={kind,value,extra} definite={kind,value} closed
r["kind"]                            #@ ok kind
r["extra"]                           #@ possibly extra
r["nope"]                            #@ missing nope


def wrap(*a, **k):
    return inner(*a, **k)


def inner(x, y=0, **rest):
    return {"x": x, "y": y, **rest}


wrap(1, z=3)["z"]                    #@ ok z
wrap(1, 2)["y"]                      #@ ok y
wrap(1)["q"]                         #@ missing q

row = dict(zip(["id", "name"], values))
row["id"]                            #@ ok id
row["email"]                         #@ missing email
