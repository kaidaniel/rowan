# list / tuple / set / frozenset methods on values from callers: positions, membership, set algebra


def required_missing(rec, required):
    have = set(rec)
    missing = tuple(sorted(set(required) - have))
    common = have & {"id", "name"}
    return missing, common


def stack_ops():
    xs = [3, 1, 2]
    xs.append(5)
    xs.insert(0, 9)
    last = xs.pop()                              #@value last: exactly={5}
    first = xs[0]                                #@value first: exactly={9}
    i = xs.index(1)                              #@value i: exactly={2}
    xs.sort()
    t = tuple(xs)                                #@value t: exactly={(1,2,3,9)}
    a, *rest = t
    n = len(rest)                                #@value n: exactly={3}
    u = tuple(sorted({1, 2} | {2, 3}))                  #@value u: exactly={(1,2,3)}
    fs = frozenset("ab") - frozenset("b")
    one = tuple(sorted(fs))                             #@value one: exactly={("a",)}
    c = xs.count(3)                              #@value c: exactly={1}
    return xs


if __name__ == "__main__":
    m1, c1 = required_missing({"id": 1}, ["id", "name"])
    #@value m1: exactly={("name",)}
    m2, c2 = required_missing({"id": 1, "name": "a", "x": 0}, ("id",))
    #@value m2: exactly={()}
    k = tuple(sorted(c2))                               #@value k: exactly={("id","name")}
    stack_ops()
