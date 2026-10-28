"""For any call, an attribute the program's code stores outside `__init__` holds what `__init__`
gave it or what one of those stores puts there: literals (also either of a conditional's),
values of a known type (`+= 1` where every store is an int, a comparison's bool, an f-string's
str), stores by `setattr` with the name written out and from functions outside the class. A
store of anything else (a parameter, a call's result, a loop's target) leaves it unknown, as
before. A class-level default does not hide what a method stores on the instance. (Before, any
attribute a method stored was unknown beside `__init__`'s value, `0 | ?`; and one the class
defined was read as the class's value alone.)

Expectations were written from the program text before running the engine."""


class Job:
    def __init__(self):
        self.state = "new"
        self.tries = 0
        self.done = False
        self.label = None
        self.note = "x"

    def start(self):
        self.state = "running"
        self.tries += 1

    def finish(self, ok):
        self.state = "done" if ok else "failed"
        self.done = ok is True
        self.label = f"job {self.tries}"
        self.note = ok

    def report(self):
        s = self.state
        #@general s: exactly={'new', 'running', 'done', 'failed', 'poked'}
        t = self.tries
        #@general t: shows={0} types={int}
        d = self.done
        #@general d: shows={False} types={bool}
        lb = self.label
        #@general lb: shows={None} types={str}
        n = self.note
        #@general n: shows={'x'} untyped
        return s


def poke(j: Job):
    j.state = "poked"


class Cursor:
    def __init__(self):
        self.pos = 0

    def scan(self, items):
        for self.pos in items:
            pass

    def where(self):
        p = self.pos
        #@general p: shows={0} untyped
        return p


class Flag:
    def __init__(self):
        self.on = False

    def set(self):
        setattr(self, "on", True)

    def get(self):
        o = self.on
        #@general o: exactly={False, True}
        return o


class Counter:
    count = 0
    kind = "a"

    def bump(self):
        self.count += 1
        self.kind = "b"

    def show(self):
        # (the instance's own value, once a method stored it, else the class's)
        c = self.count
        #@general c: shows={0} types={int}
        k = self.kind
        #@general k: exactly={'a', 'b'}
        return c


j = Job()
j.start()
j.finish(True)
j.report()
poke(j)
j.report()
c = Cursor()
c.scan([1, 2])
c.where()
f = Flag()
f.set()
f.get()
k = Counter()
k.show()
k.bump()
k.show()
