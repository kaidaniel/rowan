# A method call past its function's budget is unknown code, but its receiver's own attributes
# change only by the names the method's code (and the methods it runs on the receiver) uses
# on it: what it stores may hold anything, what it deletes may be gone, the rest stays. Unless
# the receiver escapes (passed on, aliased) or is reachable from what the call is given: then
# anything about it may have changed. (Each method's second call is past the budget.)
#@limits budget=2


class Counter:
    def __init__(self):
        self.n = 0
        self.tag = "c"
        self.items = {}

    def bump(self, k):
        self.n = self.n + 1
        self.items[k] = 1
        self._note(k)

    def _note(self, k):
        if k == "drop":
            del self.tag


c = Counter()
c.bump("a")
c.bump("b")
c.n                                              #@ ok n
c.items                                          #@ ok items
c.tag                                            #@ possibly tag
#@todo ok tag  (k is "b": the skipped call does not delete it)


class Leaky:
    def __init__(self):
        self.a = 1

    def run(self, sink):
        sink.append(self)
        x = 1
        y = 2


registry = []
lk = Leaky()
lk.run(registry)
lk.run(registry)
lk.a                                             #@ likely a
#@todo ok a  (run does not delete it; the receiver escaped into `registry`)


class Node:
    def __init__(self, parent):
        self.parent = parent
        self.name = "n"

    def touch(self, other):
        other.count = 1
        x = 1
        y = 2


root = Node(None)
kid = Node(root)
kid.touch(root)
kid.touch(root)
root.name                                        #@ likely name
#@todo ok name  (the receiver is reachable from the argument: unknown code may change it)


REG = {}


class Global:
    def __init__(self):
        self.v = 1

    def go(self, kill):
        if kill:
            for o in REG.values():
                del o.v
        x = 1
        y = 2


gl = Global()
REG["g"] = gl
gl.go(False)
gl.go(True)
gl.v                                             #@ likely v
#@todo missing v  (the skipped call deletes it through REG)


class Hooked:
    def __init__(self):
        self.w = 1
        self.handler = self._drop

    def _drop(self):
        del self.w

    def fire(self, n):
        if n > 1:
            self.handler()
        x = 1
        y = 2


hk = Hooked()
hk.fire(0)
hk.fire(2)
hk.w                                             #@ likely w
#@todo missing w  (the skipped call runs a bound method of the receiver held in an attribute)
