"""Nested dicts updated through path helpers: `set_path` walks `path[:-1]` creating levels
with setdefault, `get_path` walks with membership checks and a default.

Expectations were written from the program text before running the engine."""


def set_path(d, path, value):
    cur = d
    for k in path[:-1]:
        cur = cur.setdefault(k, {})
    cur[path[-1]] = value
    return d


def get_path(d, path, default=None):
    cur = d
    for k in path:
        if k not in cur:
            return default
        cur = cur[k]
    return cur


cfg = {}
set_path(cfg, ["db", "host"], "h")
set_path(cfg, ["db", "port"], 1)
set_path(cfg, ["log", "level"], "x")
set_path(cfg, ["log", "handlers", "file"], "/tmp/x")
_ = 0                                     #@row cfg: present={db,log} definite={db,log} closed
db = cfg["db"]                            #@row db: present={host,port} definite={host,port} closed
log = cfg["log"]                          #@row log: present={level,handlers} definite={level,handlers} closed
handlers = log["handlers"]                #@row handlers: present={file} definite={file} closed
host = get_path(cfg, ["db", "host"])
user = get_path(cfg, ["db", "user"], "none")
cfg["db"]["user"]                         #@ missing user
cfg["log"]["level"]                       #@ ok level
