"""A method past its budget whose receiver does not escape reaches through the receiver only
the attributes its code names: what the receiver's other attributes hold keeps its keys and
values. Every attribute's objects were havocked: rich's `Console.is_terminal`, skipped 1658
times, walked the whole console each time."""
#@limits budget=2


class Console:
    def __init__(self):
        self.theme = {"red": 1, "blue": 2}
        self.size = [80, 24]
        self.force = None

    def is_terminal(self):
        if self.force is not None:
            return self.force
        self.size.append(0)
        return self.size[0] > 0


c = Console()
t1 = c.is_terminal()
t2 = c.is_terminal()
t3 = c.is_terminal()
red = c.theme["red"]                             #@ ok red
#@value red: exactly={1}
w = c.size[0]
#@value w: includes={80}


class Counter:
    total = {"n": 0}

    def bump(self, v):
        a = 1
        b = 2
        type(self).total["n"] = v


k = Counter()
k.bump(1)
k.bump(2)
k.bump(3)
tn = Counter.total["n"]
#@value tn: includes={3}
