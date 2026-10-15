# A function that defines a generator function inside is not a generator itself: a call runs
# its body (a class decorator adding a generator method returns the class; a factory returns
# its dict).


def decorate(cls):
    def items(self):
        yield self.a
        yield self.b

    cls.items = items
    return cls


@decorate
class P:
    def __init__(self, a, b):
        self.a = a
        self.b = b


p = P(1, 2)
x = p.a  #@value x: exactly={1}
vals = list(p.items())
n = len(vals)  #@value n: exactly={2}


def make():
    def gen():
        yield 1

    return {"g": gen, "k": 3}


d = make()  #@row d: present={g,k} definite={g,k} closed
k = d["k"]  #@ ok k
#@value k: exactly={3}
