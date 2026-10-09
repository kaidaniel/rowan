# contextlib.ExitStack: entered contexts and registered callbacks run in reverse order at exit, on the same dicts

from contextlib import ExitStack, contextmanager

EVENTS = []


def close(res, reason):
    res["closed"] = reason
    EVENTS.append(res["name"])
#@schema param close.res: always={name} only={name}


@contextmanager
def opened(name):
    res = {"name": name, "open": True}
    yield res
    res["open"] = False


def run(names):
    with ExitStack() as stack:
        handles = [stack.enter_context(opened(n)) for n in names]
        for h in handles:
            stack.callback(close, h, reason="done")
        first = handles[0] if handles else {"name": "-"}
        #@row first: present={name,open} definite={name} closed
    return handles


hs = run(["a", "b"])
h0 = hs[0]                                       #@row h0: present={name,open,closed} definite={name,open,closed} closed
o = h0["open"]                                   #@value o: exactly={False}
r = h0["closed"]                                 #@value r: exactly={"done"}
ev = "|".join(EVENTS)                            #@value ev: exactly={"b|a"}
empty = run([])
ne = len(empty)                                  #@value ne: exactly={0}
