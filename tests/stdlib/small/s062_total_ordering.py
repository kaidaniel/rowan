# functools.total_ordering derives <=, >, >= from __eq__ and __lt__; sorted / max use them

from functools import total_ordering


@total_ordering
class Version:
    def __init__(self, text):
        major, minor = text.split(".")
        self.major = int(major)
        self.minor = int(minor)

    def __eq__(self, other):
        return (self.major, self.minor) == (other.major, other.minor)

    def __lt__(self, other):
        return (self.major, self.minor) < (other.major, other.minor)


def latest(release_rows):
    vs = [Version(r["version"]) for r in release_rows]
    top = max(vs)
    #@row top: present={major,minor} definite={major,minor} closed
    return top.major
#@schema param latest.release_rows['*']: always={version} only={version}


a = Version("1.2")                       #@row a: present={major,minor} definite={major,minor} closed
b = Version("1.10")
gt = b > a                               #@value gt: exactly={True}
le = a <= b                              #@value le: exactly={True}
ge = a >= b                              #@value ge: exactly={False}
eq = a == Version("1.2")                 #@value eq: exactly={True}
newest = max([a, b, Version("0.9")])
mj = newest.minor                        #@value mj: exactly={10}
srt = sorted([b, a])
fm = srt[0].minor                        #@value fm: exactly={2}
try:
    a.patch                              #@ missing patch
except AttributeError:
    pass

if __name__ == "__main__":
    m = latest([{"version": "2.0", "name": "x"}, {"version": "1.5"}])   #@value m: exactly={2}
    m2 = latest([{"version": "0.1"}])                                   #@value m2: exactly={0}
