# Code between a generator's items changes what the generator itself changes: the generator's
# later steps happen on top of that (G[0] += 10 after G[0] = 5 gives 15; D gets "c" beside the
# consumer's "z").
G = [0]
D = {"a": 0}


def bump():
    G[0] += 1
    yield G[0]
    G[0] += 10
    yield G[0]


def setd():
    D["b"] = 1
    yield 1
    D["c"] = 2
    yield 2


g = bump()
x = next(g)                                  #@value x: exactly={1}
G[0] = 5
y = next(g)                                  #@value y: includes={15}
c = G[0]                                     #@value c: includes={15}
#@todo value c: exactly={15}  (the generator's run on the changed item is not taken key by key)
h = setd()
next(h)
D["z"] = 0
next(h)
d = D                                        #@row d: possible={a, b, c, z} definite_within={a, b, c, z}
#@todo row d: present={a, b, c, z} definite={a, b, c, z} closed  (joined with the state before the change)
