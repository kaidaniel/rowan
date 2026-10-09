# a module dict that functions store into: each call sees the dict as it is at its call site
DB = {1: {"id": 1, "name": "ann"}}
SEEN = {}


def create(body):
    new_id = max(DB) + 1                             #@value new_id: exactly={2,3}
    DB[new_id] = {"id": new_id, "name": body["name"]}
    return DB[new_id]


def read(tid):
    if tid not in DB:
        return {"error": tid}
    SEEN[tid] = True
    return DB[tid]


a = create({"name": "pen"})
#@row a: present={id,name} definite={id,name} closed
got = read(2)
#@row got: present={id,name} definite={id,name} closed
miss = read(9)
#@row miss: present={error} definite={error} closed
b = create({"name": "cup"})
n = len(DB)                                          #@value n: exactly={3}
#@row DB: present={1,2,3} definite={1,2,3} closed
#@row SEEN: present={2} definite={2} closed
