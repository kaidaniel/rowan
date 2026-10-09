# dict `|` and `|=`, `{**a, **b}` and dict(a, **kw) in helpers: union of both sides' keys


def layer(base, override):
    merged = base | override
    _ = 0                                        #@row merged: present={host,port,debug,user} definite={host,port} closed
    merged["host"]                               #@ ok host
    return merged


def patch(cfg, flag):
    cfg |= {"patched": True}
    if flag:
        cfg |= {"debug": True}
    _ = 0                                        #@row cfg: present={host,port,debug,user,patched} definite={host,port,patched} closed
    cfg["patched"]                               #@ ok patched
    return cfg


def spread(a, b):
    out = {**a, "via": "spread", **b}
    _ = 0                                        #@row out: present={host,port,debug,patched,via,extra} definite={host,port,via} closed
    out2 = dict(a, source="kw")
    _ = 0                                        #@row out2: present={host,port,debug,patched,source} definite={host,port,source} closed
    n = len(out2) - len(a)                       #@value n: exactly={1}
    return out, out2


def debug_of(cfg):
    return cfg["debug"]                          #@ ok debug ; missing debug


if __name__ == "__main__":
    m1 = layer({"host": "h", "port": 1}, {})
    m2 = layer({"host": "h"}, {"port": 2, "debug": False})
    m3 = layer({"host": "h", "port": 3}, {"user": "u"})
    p1 = patch(m1, False)
    p2 = patch(m3, True)
    spread(p1, {})
    spread(m2, {"extra": 1})
    for c in (p1, p2, m2):
        try:
            debug_of(c)
        except KeyError:
            pass
