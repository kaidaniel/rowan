# functools.partial binding a dict positionally: the bound function sees that dict at every call

from functools import partial


def lookup(table, key):
    return table[key]


def fill(template, record):
    out = dict(template)
    out.update(record)
    return out


def run_lookups(table):
    get = partial(lookup, table)
    a = get("host")                               #@ ok host
    b = get("port")                               #@ possibly port
    return a, b
#@schema param run_lookups.table: always={host,port} only={host,port}


defaults = {"host": "localhost", "port": 80, "debug": False}
with_defaults = partial(fill, defaults)
cfg = with_defaults({"port": 8080, "user": "x"})  #@row cfg: present={host,port,debug,user} definite={host,port,debug,user} closed
p = cfg["port"]                                    #@value p: exactly={8080}
h = with_defaults({})["host"]                      #@value h: exactly={"localhost"}
n = len(with_defaults.args)                        #@value n: exactly={1}
d2 = defaults                                      #@row d2: present={host,port,debug} definite={host,port,debug} closed

if __name__ == "__main__":
    r = run_lookups({"host": "h", "port": 1, "tls": True})
    try:
        run_lookups({"host": "h2"})
    except KeyError:
        pass
