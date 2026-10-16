"""A `for` over an instance whose `__next__` is not decided call by call (its count is an
input) calls `__next__` once per item, any number of times: its effects add up. rowan applied
`__next__` once (or not at all): `c.i` was 0 or 1 after the loop and an attribute the second
call sets was reported definitely missing. A `__next__` decided call by call (one item, then
StopIteration) ran before the loop: the items the body added were not taken.

Expectations were written from the program text before running the engine."""
import os


class Counter:
    def __init__(self, n):
        self.n = n
        self.i = 0

    def __iter__(self):
        return self

    def __next__(self):
        if self.i >= self.n:
            raise StopIteration
        self.i += 1
        if self.i == 2:
            self.second = True
        return self.i


def count(n):
    c = Counter(n)
    total = 0
    for x in c:
        total += x
    print(c.second)                      #@ possibly second
    return c.i


class Queue:
    def __init__(self):
        self.items = [1]
        self.pos = 0

    def __iter__(self):
        return self

    def __next__(self):
        if self.pos >= len(self.items):
            raise StopIteration
        x = self.items[self.pos]
        self.pos += 1
        if self.pos == 3:
            self.third = True
        return x


def walk(n):
    # the loop's body gives the iterator more items: `__next__` runs after each iteration
    q = Queue()
    for x in q:
        if x < n:
            q.items.append(x + 1)
    print(q.third)                       #@ possibly third
    return q.pos


i = count(int(os.environ.get("Z547_N", "3")))
#@value i: unknown
try:
    count(int(os.environ.get("Z547_M", "0")))
except AttributeError:
    pass
p = walk(int(os.environ.get("Z547_N", "3")))
#@value p: unknown
