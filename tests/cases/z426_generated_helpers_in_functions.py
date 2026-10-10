# itemgetter / attrgetter / re.match first used inside a function body (their small generated modules)
import re
from operator import attrgetter, itemgetter


class P:
    def __init__(self, n):
        self.n = n


def team_of(rec):
    k = itemgetter("team")
    return k(rec)


def name_of(p):
    return attrgetter("n")(p)


def level(line):
    m = re.match(r"(?P<lvl>[A-Z]+): (?P<msg>.*)", line)
    return m.groupdict() if m else {}


t = team_of({"team": "r", "x": 1})                  #@value t: exactly={r}
n = name_of(P("ann"))                                #@value n: exactly={ann}
g = level("WARN: disk")
#@row g: present={lvl,msg} closed
k2 = [itemgetter("a")(d) for d in [{"a": 1}, {"a": 2}]]
s = k2[1]                                            #@value s: exactly={2}
