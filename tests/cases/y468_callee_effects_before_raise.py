"""What a callee did before it raised is there in the caller's handler. A summarised callee that
may raise was taken to reach the handler from the state before the call: its effects were lost
(`d["a"]` read as definitely missing after `f` set it and raised; a callee's finally that
closes something, likewise). A call that may raise into a handler of the caller runs per call
site, where the state at each raise is known."""


def f(d):
    d["a"] = 1
    raise ValueError("x")


def g(log):
    try:
        log["x"].append(1)
    finally:
        log["closed"] = True


d = {}
try:
    f(d)
except ValueError:
    pass
print(d["a"])  #@row d: present={a} definite={a} closed
log = {}
try:
    g(log)
except KeyError:
    pass
print(log["closed"])  #@row log: present={closed} definite={closed} closed
