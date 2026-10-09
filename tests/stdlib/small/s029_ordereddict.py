# OrderedDict move_to_end / popitem(last=False) as an LRU cache; move_to_end of an absent key raises
from collections import OrderedDict


def touch(cache, key):
    cache.move_to_end(key)                       #@ ok a
    return cache


def evict(cache, cap):
    gone = []
    while len(cache) > cap:
        k, _v = cache.popitem(last=False)
        gone.append(k)
    return tuple(gone)


if __name__ == "__main__":
    od = OrderedDict(a=1, b=2, c=3)
    touch(od, "a")
    order = tuple(od)                            #@value order: exactly={("b","c","a")}
    try:
        od.move_to_end("zz")                     #@ missing zz
    except KeyError:
        pass
    od.move_to_end("c", last=False)
    k, v = od.popitem(last=False)
    #@value k: exactly={"c"}
    _ = 0                                        #@row od: present={a,b} definite={a,b} absent={c} closed
    last = od.popitem()[0]                       #@value last: exactly={"a"}
    cache = OrderedDict([("x", 1), ("y", 2), ("z", 3)])
    g = evict(cache, 1)                          #@value g: exactly={("x","y")}
    _ = 0                                        #@row cache: present={z} definite={z} absent={x,y} closed
