# contextlib.closing calls close() on exit; nullcontext(x) yields x itself

from contextlib import closing, nullcontext


class Conn:
    def __init__(self, opts):
        self.opts = opts
        self.state = {"open": True, "host": opts["host"]}

    def close(self):
        self.state["open"] = False
        self.state["closed_by"] = "closing"
#@schema param Conn.__init__.opts: always={host} only={host}


def use(opts):
    with closing(Conn(opts)) as c:
        st = c.state
        #@row st: present={open,host} definite={open,host} closed
    fin = dict(st)                                #@row fin: present={open,host,closed_by} definite={open,host,closed_by} closed
    return fin["open"]                            #@ ok open


def scoped(cfg, use_copy):
    cm = nullcontext(dict(cfg)) if use_copy else nullcontext(cfg)
    with cm as c:
        c["touched"] = True
    return cfg


o = use({"host": "db"})                           #@value o: exactly={False}
o2 = use({"host": "db2", "port": 5})              #@value o2: exactly={False}
c = Conn({"host": "h"})                           #@row c: present={opts,state} definite={opts,state} closed
a = scoped({"x": 1}, True)                        #@row a: present={x} definite={x} closed
b = scoped({"x": 1}, False)                       #@row b: present={x,touched} definite={x,touched} closed
with nullcontext() as nothing:                    #@value nothing: exactly={None}
    pass
