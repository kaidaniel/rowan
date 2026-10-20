# A call past its budget of a function that only gives back an attribute of its receiver or of
# an argument (a getter: `return self._link`, `return p.name`, `return self.inner.v`) is that
# attribute as the call site's object has it: the same value, the same object (a list it hands
# out and the caller changes is the receiver's). (With `budget=0` a function's first call runs,
# the later ones are past its budget.)
#@limits budget=0


class Style:
    def __init__(self, link):
        self._link = link

    @property
    def link(self):
        return self._link


class Box:
    def __init__(self):
        self.items = [1, 2]

    def get(self):
        """The items."""
        return self.items


class Inner:
    def __init__(self):
        self.v = 7


class Outer:
    def __init__(self):
        self.inner = Inner()

    def value(self):
        return self.inner.v


class P:
    def __init__(self, n):
        self.name = n


def name_of(p):
    return p.name


s = Style("http://a")
a = s.link
b = s.link
#@value b: exactly={"http://a"}
x = Box()
y0 = x.get()
y = x.get()
y.append(3)
n = len(x.items)
#@value n: exactly={3}
o = Outer()
v0 = o.value()
v = o.value()
#@value v: exactly={7}
p = P("x")
n1 = name_of(p)
n2 = name_of(p)
#@value n2: exactly={"x"}
