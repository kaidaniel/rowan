# A site past its objects (512 contexts here: the nested unrolled loops) has one summary object
# for every object it made. The object a call makes there is still one object until that call
# returns: its own stores are strong, and it joins the summary with every attribute it got, so
# what every call stores is there on every object of the site: a recursive call's too. One a
# raise leaves half-built joins it as it is where something holds it (`held`), else it is gone.
class D:
    def special(self):
        return self._sp
        #@ ok _sp


def mk(s):
    self = object.__new__(D)
    self._s = s
    self._sp = False
    n = self._s
    #@ ok _s
    return self


held = []


class E:
    pass


def mk2(s):
    self = object.__new__(E)
    held.append(self)
    self.a = 1
    if s == 3:
        raise ValueError("half-built")
    self.b = 2
    return self


class N:
    pass


def chain(n):
    node = object.__new__(N)
    node.v = n
    if n > 0:
        node.next = chain(n - 1)
    return node


for i in range(16):
    for j in range(16):
        for k in range(2):
            x = mk(i)
            z = mk2(i + 10)
            c0 = chain(0)
y = mk(5)
y._s
#@ ok _s
y.special()
#@row y: present={_s,_sp} definite={_s,_sp} closed
c = chain(2)
c.v
#@ ok v
c.next.v
#@ ok v
c2 = c.next.next
c2.v
#@ ok v
try:
    mk2(3)
except ValueError:
    pass
v = mk2(4)
v.b
#@todo ok b (v is the site's summary, which stands for the half-built object `held` keeps too)
v.a
#@ ok a
half = held[-2]
half.a
#@ ok a
half.b
#@ fails b
