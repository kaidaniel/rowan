"""Hypothesis: dict.update takes a mapping or an iterable of pairs plus keyword arguments in one
call: the pairs are applied first, in order (a later pair overwrites an earlier one), then the
keywords (a keyword wins over a pair with the same key); any 2-item sequence is a pair (the
string "ab" gives key "a"); an object with a keys() method is read as a mapping through its
__getitem__ (only the keys it lists). Text from the environment gives an open row, but the
keyword key is definite with its exact value."""
import os


class Lookup:
    def __init__(self, data):
        self.data = data

    def keys(self):
        return [k for k in self.data if not k.startswith("_")]

    def __getitem__(self, k):
        return self.data[k].upper()


def pairs_of(text):
    return [tuple(p.split("=", 1)) for p in text.split(",") if "=" in p]


def configure(base, text, **kw):
    out = dict(base)
    out.update(pairs_of(text), **kw)
    return out


def from_lookup(data):
    d = {"z": "keep"}
    d.update(Lookup(data))
    return d


def from_strings(items):
    d = {}
    d.update(items)
    return d


if __name__ == "__main__":
    c1 = configure({"a": "0"}, "b=1,c=2,b=3", c="k")
    #@row c1: present={a,b,c} definite={a,b,c} closed
    b = c1["b"]                                  #@value b: exactly={"3"}
    c = c1["c"]                                  #@value c: exactly={"k"}
    c2 = configure({}, "x,y=")
    #@row c2: present={y} definite={y} absent={x} closed
    y = c2["y"]                                  #@value y: exactly={""}
    fl = from_lookup({"p": "a", "_h": "b"})
    #@row fl: present={z,p} definite={z,p} absent={_h} closed
    fp = fl["p"]                                 #@value fp: exactly={"A"}
    fs = from_strings(["ab", "cd", "ae"])
    #@row fs: present={a,c} definite={a,c} closed
    fa = fs["a"]                                 #@value fa: exactly={"e"}
    c3 = configure({"a": "0"}, os.environ.get("S122_T", "k=v,m=n"), a="1")
    #@row c3: present={a} definite={a} open
    a3 = c3["a"]                                 #@value a3: exactly={"1"}
