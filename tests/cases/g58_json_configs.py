"""Config loading patterns: defaults merged with a file, nested sections, env overrides."""
import json
import os

DEFAULTS = {"debug": False, "db": {"host": "localhost", "port": 5432}}


def load(path):
    cfg = dict(DEFAULTS)
    with open(path) as f:
        cfg.update(json.load(f))
    if os.environ.get("APP_DEBUG"):
        cfg["debug"] = True
    cfg["db"]["host"]                    #@ input host
    cfg["db"]["user"]                    #@ possibly user
    cfg["db"]["password"]                #@ possibly password
    return cfg


cfg = load("x.json")
cfg["debug"]                             #@ ok debug
cfg["db"]                                #@ ok db
cfg["db"]["port"]                        #@ input port
cfg["log_level"]                         #@ input log_level
section = cfg.get("cache", {})
section["ttl"]                           #@ possibly ttl
# "db" comes from DEFAULTS (proven), so only log_level is required of the file (L15-16 possibly, was input: the file's db or DEFAULTS' without user)
#@schema json.load@L11: always={log_level}
#@schema json.load@L11['db']: always={user,password}


def env_override(cfg):
    for key in ("HOST", "PORT"):
        val = os.environ.get(key)
        if val is not None:
            cfg[key.lower()] = val
    return cfg


e = env_override({"host": "h"})
e["host"]                                #@ ok host
e["port"]                                #@ possibly port
e["user"]                                #@ missing user

raw = json.loads(text)
for item in raw["items"]:                #@ input items
    item["id"]                           #@ input id
    item.get("name")                     #@ none name
#@schema json.loads@L45['items']['*']: always={id}
