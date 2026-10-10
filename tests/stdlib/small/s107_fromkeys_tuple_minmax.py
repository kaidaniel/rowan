"""dict.fromkeys iterates its argument (a string's characters, in order, duplicates once); an
iterable of unknown length gives possible keys only; min / max compare tuples and pairs item by
item (first extreme wins ties), also with a key function returning tuples."""
import sys


def first_seen(text):
    return tuple(dict.fromkeys(text))


if __name__ == "__main__":
    a = first_seen("abca")                       #@value a: exactly={("a","b","c")}
    b = tuple(dict.fromkeys(["b", "a", "b"], 0))   #@value b: exactly={("b","a")}
    xs = [x for x in ["p", "q"] if x in sys.argv]
    c = dict.fromkeys(xs)                        #@row c: present={p,q} definite={} closed
    m1 = min(zip([3, 1], "ab"))                  #@value m1: exactly={(1,"b")}
    m2 = max([(1, "z"), (1, "a"), (0, "q")])     #@value m2: exactly={(1,"z")}
    d = {"x": 2, "y": 2, "w": 1}
    m3 = max(d.items(), key=lambda kv: (kv[1], kv[0]))   #@value m3: exactly={("y",2)}
    m4 = min(enumerate([5, 3, 3]), key=lambda p: p[1])   #@value m4: exactly={(1,3)}
