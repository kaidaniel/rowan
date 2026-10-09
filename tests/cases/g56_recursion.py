"""Recursion building and walking records."""


def build(depth):
    node = {"depth": depth}
    if depth > 0:
        node["child"] = build(depth - 1)
    return node


t = build(2)
t["depth"]                               #@ ok depth
# 2 > 0 is decided: the top node has a child (the recursion bottoms out at depth 0).
t["child"]                               #@ ok child
t["child"]["depth"]                      #@ ok depth
t["parent"]                              #@ missing parent


def walk(node, acc):
    acc["visited"] = acc.get("visited", 0) + 1
    if "child" in node:
        walk(node["child"], acc)
    return acc


w = walk(t, {})
w["visited"]                             #@ ok visited
w["other"]                               #@ missing other


def flatten(d, prefix=""):
    out = {}
    for k, v in d.items():
        if isinstance(v, dict):
            out.update(flatten(v, prefix + k + "."))
        else:
            out[prefix + k] = v
    return out


flat = flatten({"a": 1, "b": {"c": 2}})
flat["a"]                                #@ ok a
flat["q"]                                #@ missing q


def ping(n, log):
    log["ping"] = True
    if n == 0:
        return log
    return pong(n - 1, log)


def pong(n, log):
    log["pong"] = True
    if n == 0:
        return log
    return ping(n - 1, log)


pl = ping(3, {})
pl["ping"]                               #@ ok ping
# n == 3 is a literal and `n == 0` narrows it away, so the pong path is the only one
pl["pong"]                               #@ ok pong
pl["zip"]                                #@ missing zip


def fact(n):
    if n <= 1:
        return {"n": 1}
    prev = fact(n - 1)
    return {"n": n * prev["n"]}          #@ ok n


fact(5)["n"]                             #@ ok n
fact(5)["m"]                             #@ missing m
