import json
import lib

cfg = json.load(fp)                  #@row cfg: present={} definite={} open
cfg = lib.apply_defaults(cfg)
cfg["timeout"]                       #@ input timeout
#@schema json.load@L4: always={timeout}

opts = {"host": "db", "port": 5432}
lib.apply_defaults(opts)             #@row opts: present={host,port} definite={} open
opts["host"]                         #@ likely host
opts["user"]                         #@ likely-missing user
