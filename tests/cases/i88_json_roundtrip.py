"""JSON round trip: a record packed with json.dumps and read back with json.loads keeps its
keys (written as an open external input with a schema while dumps/loads were opaque).

Expectations were written from the program text before running the engine."""
import json


def pack(rec):
    return json.dumps(rec)


def unpack(s):
    return json.loads(s)


rec = {"id": 1, "tags": ["a"], "meta": {"k": 1}}
back = unpack(pack(rec))                  #@row back: present={id,tags,meta} definite={id,tags,meta} closed
back["id"]                                #@ ok id
back["tags"]                              #@ ok tags
meta = back["meta"]                       #@row meta: present={k} definite={k} closed
meta["k"]                                 #@ ok k
rec["id"]                                 #@ ok id
rec["zz"]                                 #@ missing zz
# (was: schema json.loads@L13: always={id,tags,meta}, ['meta']: always={k};
# the text is the program's own data now: no input)
