"""A store by a name the text does not say (`setattr(o, name, v)`, a `__dict__` changed) reaches
what the text says it is made on (z914, z916): the method's receiver (its class and subclasses), a
parameter annotated with a class, an instance `C()` made there; other classes' attributes hold
what the program's stores by name put there. A `__dict__` only read (`len`, `in`, a subscript compared)
stores nothing; one changed through a local bound to it does. (Before, one such store
anywhere made every attribute a method stores hold anything.)

Expectations were written from the program text before running the engine."""


class Options:
    def __init__(self):
        self.verbose = False

    def load(self, pairs):
        for k, v in pairs:
            setattr(self, k, v)

    def show(self):
        v = self.verbose
        #@general v: untyped
        return v


class SubOptions(Options):
    def __init__(self):
        super().__init__()
        self.level = 0

    def raise_level(self):
        self.level = 1

    def lvl(self):
        # (`load` may run on a SubOptions: any name)
        x = self.level
        #@general x: untyped
        return x


class Job:
    def __init__(self):
        self.state = "new"

    def start(self):
        self.state = "running"

    def size(self):
        return len(self.__dict__)

    def has(self, k):
        return k in self.__dict__ and self.__dict__[k] is not None

    def report(self):
        s = self.state
        #@general s: exactly={'new', 'running'}
        return s


class Loose:
    def __init__(self):
        self.mode = "a"

    def set_mode(self):
        self.mode = "b"

    def get(self):
        m = self.mode
        #@general m: untyped
        return m


def fill(target: Loose, data):
    d = target.__dict__
    d.update(data)


class Record:
    def __init__(self):
        self.kind = "r"

    def retag(self):
        self.kind = "s"

    def get(self):
        kk = self.kind
        #@general kk: untyped
        return kk


def from_pairs(pairs):
    r = Record()
    for k, v in pairs:
        setattr(r, k, v)
    return r


class Plain:
    def __init__(self):
        self.n = 0

    def bump(self):
        self.n = 1

    def get(self):
        y = self.n
        #@general y: exactly={0, 1}
        return y


o = Options()
o.load([("verbose", "yes")])
o.show()
so = SubOptions()
so.load([("level", "high")])
so.lvl()
so.raise_level()
j = Job()
j.start()
j.size()
j.has("state")
j.report()
lo = Loose()
fill(lo, {"mode": 3})
lo.get()
lo.set_mode()
r = from_pairs([("kind", None)])
r.get()
r.retag()
p = Plain()
p.bump()
p.get()
