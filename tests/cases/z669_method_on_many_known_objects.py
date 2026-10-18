# A method called on each of many known objects (scipy's `{nfo.func.__name__: nfo.wrapper for
# nfo in _special_funcs}` over 91 entries, each parsing a docstring). Past the unrolling bound,
# a fixpoint over the items' join ran the method once per object in every round: the first
# pass, then each round until the registry stopped growing (6 x 91 parses). Each item one
# distinct object: unrolled, once per object, in order.
import sys


def parse(doc):
    out = []
    for line in doc.split("\n"):
        line = line.strip()
        if line.startswith("Note"):
            out.append(line)
    return out


registry = {}


class Info:
    def __init__(self, name, doc):
        self.name = name
        self.doc = doc

    def wrap(self):
        registry[self.name] = parse(self.doc)
        return self.name.upper()


infos = (
    Info("a", sys.argv[0]),
    Info("b", sys.argv[0]),
    Info("c", sys.argv[0]),
    Info("d", sys.argv[0]),
    Info("e", sys.argv[0]),
    Info("f", sys.argv[0]),
    Info("g", sys.argv[0]),
    Info("h", sys.argv[0]),
    Info("i", sys.argv[0]),
    Info("j", sys.argv[0]),
    Info("k", sys.argv[0]),
    Info("l", sys.argv[0]),
    Info("m", sys.argv[0]),
    Info("n", sys.argv[0]),
    Info("o", sys.argv[0]),
    Info("p", sys.argv[0]),
    Info("q", sys.argv[0]),
    Info("r", sys.argv[0]),
    Info("s", sys.argv[0]),
    Info("t", sys.argv[0]),
    Info("u", sys.argv[0]),
    Info("v", sys.argv[0]),
    Info("w", sys.argv[0]),
    Info("x", sys.argv[0]),
)
names = {nfo.name: nfo.wrap() for nfo in infos}
first = names["a"]
#@value first: exactly={"A"}
last = names["x"]
#@value last: exactly={"X"}
#@row registry: present={"a","b","c","d","e","f","g","h","i","j","k","l","m","n","o","p","q","r","s","t","u","v","w","x"} definite={"a","b","c","d","e","f","g","h","i","j","k","l","m","n","o","p","q","r","s","t","u","v","w","x"} closed
#@work <= 450
