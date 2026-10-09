# appends in branches and try bodies: each path keeps its positions; after the join the shared prefix stays
import json


def collect(flag, raw):
    out = [{"k": "first"}]
    if flag:
        out.append({"k": "flag", "x": 1})
    n = len(out)                                     #@value n: exactly={1,2}
    head = out[0]
    #@row head: present={k} definite={k} closed
    try:
        data = json.loads(raw)
        out.append({"k": "json", "data": data})
    except ValueError:
        out.append({"k": "bad"})
    last = out[-1]
    #@row last: present={k,data} definite={k} closed
    return out


def straight():
    out = []
    try:
        out.append({"a": 1})
        out.append({"b": 2})
    except KeyError:
        pass
    second = out[1]
    #@row second: present={b} definite={b} closed
    m = len(out)                                     #@value m: exactly={2}
    return m


r1 = collect(True, "1")
r2 = collect(False, "{")
s = straight()                                       #@value s: exactly={2}
