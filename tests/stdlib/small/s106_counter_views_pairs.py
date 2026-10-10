"""Counter / dict views and pairs keep insertion order and contents: Counter.items / keys /
values (a Counter's missing-key count is not a key), most_common pairs (distinct tuples, ties in
insertion order), list(d.items()), zip and enumerate pairs; inside a function too."""
from collections import Counter


def kind_counts(entries):
    c = Counter(e["k"] for e in entries)
    return tuple(c.most_common())


if __name__ == "__main__":
    c = Counter("abcb")
    items = tuple(c.items())                     #@value items: exactly={(("a",1),("b",2),("c",1))}
    keys = tuple(c.keys())                       #@value keys: exactly={("a","b","c")}
    vals = tuple(c.values())                     #@value vals: exactly={(1,2,1)}
    mc = tuple(c.most_common())                  #@value mc: exactly={(("b",2),("a",1),("c",1))}
    top = c.most_common(1)[0]                    #@value top: exactly={("b",2)}
    es = [{"k": "INV"}, {"k": "TRIP"}, {"k": "NONE"}, {"k": "TRIP"}, {"k": "INV"}, {"k": "NONE"}, {"k": "NONE"}]
    kc = kind_counts(es)                         #@value kc: exactly={(("NONE",3),("INV",2),("TRIP",2))}
    d = {"x": 1, "y": 2}
    pairs = tuple(list(d.items()))               #@value pairs: exactly={(("x",1),("y",2))}
    z = tuple(zip([1, 2], "ab"))                 #@value z: exactly={((1,"a"),(2,"b"))}
    en = tuple(enumerate(["p", "q"], 1))         #@value en: exactly={((1,"p"),(2,"q"))}
    n = len(set("aab"))                          #@value n: exactly={2}
