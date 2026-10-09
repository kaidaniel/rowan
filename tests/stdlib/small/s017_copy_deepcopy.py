# copy.copy shares nested dicts, copy.deepcopy does not: which record a later mutation reaches
import copy

BASE = {"db": {"host": "h", "port": 1}, "tags": ["a"]}


def variant(base, port):
    shallow = copy.copy(base)
    deep = copy.deepcopy(base)
    deep["db"]["port"] = port
    deep["db"]["user"] = "u"
    shallow["name"] = "s"
    _ = 0                                        #@row shallow: present={db,tags,name} definite={db,name} closed
    inner = deep["db"]                           #@row inner: present={host,port,user} definite={host,port,user} closed
    orig = base["db"]                            #@row orig: present={host,port} definite={host,port} closed
    shallow["db"]["extra"] = 1
    after = base["db"]                           #@row after: present={host,port,extra} definite={host,port,extra} closed
    b = base.get("name")                         #@value b: exactly={None}
    return deep


if __name__ == "__main__":
    d1 = variant(copy.deepcopy(BASE), 2)
    d2 = variant({"db": {"host": "x", "port": 3}}, 4)
    n = BASE["db"].get("extra")                  #@value n: exactly={None}
    p = d1["db"]["port"]                         #@value p: exactly={2}
    lst = BASE["tags"].copy()
    lst.append("b")
    n2 = len(BASE["tags"])                       #@value n2: exactly={1}
    try:
        d2["tags"]                               #@ missing tags
    except KeyError:
        pass
