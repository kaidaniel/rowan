"""JSON round trip: a record packed with json.dumps and read back with json.loads is an
external input again; what the program reads from it becomes a schema.

Expectations were written from the program text before running the engine."""
import json


def pack(rec):
    return json.dumps(rec)


def unpack(s):
    return json.loads(s)


rec = {"id": 1, "tags": ["a"], "meta": {"k": 1}}
back = unpack(pack(rec))                  #@row back: present={} definite={} open
back["id"]                                #@ input id
back["tags"]                              #@ input tags
meta = back["meta"]                       #@row meta: present={} definite={} open
meta["k"]                                 #@ input k
rec["id"]                                 #@ ok id
rec["zz"]                                 #@ missing zz
#@schema json.loads@L13: always={id,tags,meta}
#@schema json.loads@L13['meta']: always={k}
