import json

with open("cfg.json") as f:
    cfg = json.load(f)

db = cfg["db"]                                  #@ input db
host = db["host"]                               #@ input host
port = cfg["db"]["port"]                        #@ input port
ttl = cfg.get("cache", {}).get("ttl")           #@ none ttl
cfg["db"]["user"] = "x"
cfg["db"]["user"]                               #@ ok user
del cfg["db"]["host"]
cfg["db"]["host"]                               #@ missing host
#@schema json.load@L4['db']: always={host,port}
#@schema json.load@L4['cache']: none
