"""`-x` runs `x.__neg__`, `xs[x]` runs `x.__index__`, `1 in x` runs `x.__contains__`, `del x.a`
runs `x.__delattr__`: each once, logging its name."""
log = []


class Num:
    def __init__(self, v):
        self.v = v

    def __neg__(self):
        log.append("neg")
        return -self.v

    def __index__(self):
        log.append("index")
        return self.v

    def __contains__(self, x):
        log.append("contains")
        return x == self.v

    def __delattr__(self, name):
        log.append("delattr")
        object.__delattr__(self, name)


n = Num(1)
a = -n
#@value a: exactly={-1}
b = [5, 6, 7][n]
#@value b: exactly={6}
c = 1 in n
#@value c: exactly={True}
n.extra = 2
del n.extra
k = len(log)
#@value k: exactly={4}
last = log[-1]
#@value last: exactly={"delattr"}
