# A function's values for any call (its general run: open parameters of their annotated types)
# are kept apart from what the program's own calls give (`#@general` / `#@value`).


def pick(x: int, b: bool):
    y = x if b else 3
    #@value y: exactly={3, 7}
    #@general y: includes={3} unknown
    return y


def double(n: int):
    m = n * 2
    #@value m: exactly={10, 14}
    #@general m: unknown
    return m


def label(code: int):
    word = "one" if code == 1 else "many"
    #@value word: exactly={one}
    #@general word: exactly={one, many}
    return word


def never_called(k: int):
    z = k + 1
    #@value z: unknown
    #@general z: unknown
    return z


class Counter:
    def __init__(self, start: int = 0):
        self.n = start

    def get(self):
        v = self.n
        #@value v: exactly={4}
        #@general v: unknown
        return v


r = pick(5, b=False)
s = pick(7, True)
t = double(5) + double(7)
w = label(1)
c = Counter(4)
got = c.get()


class Box:
    def __init__(self, size: int = 1):
        self.size = size

    def area(self):
        # (nothing calls it: its value for any Box, not only one made with the default size)
        a = self.size * self.size
        #@general a: unknown
        #@general a: types={int}
        return a
