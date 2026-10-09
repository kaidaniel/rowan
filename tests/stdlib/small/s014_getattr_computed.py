# setattr / getattr with names computed from literal lists and from callers' arguments

FIELDS = ["host", "port", "user"]


class Conf:
    pass


def load(conf_dict):
    c = Conf()
    for f in FIELDS:
        setattr(c, f, conf_dict.get(f))
    _ = 0                                        #@row c: present={host,port,user} definite={host,port,user} closed
    return c


def dump(c, names):
    out = {n: getattr(c, n) for n in names}
    _ = 0                                        #@row out: present={host,port,user} definite={host} closed
    return out


def override(c, key, val):
    setattr(c, key, val)
    return c


if __name__ == "__main__":
    c = load({"host": "h", "port": 1})
    full = dump(c, FIELDS)                       #@row full: present={host,port,user} definite={host,port,user} closed
    d = dump(c, ["host"])                        #@row d: present={host} definite={host} closed
    c2 = override(c, "time" + "out", 3)          #@row c2: present={host,port,user,timeout} definite={host,port,user,timeout} closed
    t = getattr(c2, "timeout")                   #@value t: exactly={3}
    try:
        getattr(c2, "to" + "ken")                #@ missing token
    except AttributeError:
        pass
    p = getattr(c2, "port")                      #@value p: exactly={1}
