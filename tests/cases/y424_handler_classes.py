"""Hypothesis: each except clause starts only from the raises it catches (the first matching
clause wins, subclasses included: a NotFound(AppError, KeyError) goes to `except KeyError`); a
raise that no clause of an inner try catches reaches the outer try's handler; an implicit
ValueError from int() is an exception object of its class; a later clause that a raise never
reaches does not run."""
import sys


class AppError(Exception):
    pass


class NotFound(AppError, KeyError):
    pass


def lookup(d, k):
    if k not in d:
        raise NotFound(k)
    return d[k]


def classify(d, k):
    try:
        v = lookup(d, k)
    except KeyError:
        return "key"
    except AppError:
        return "app"
    return "ok:" + str(v)


def nested(text):
    out = {"start": 1}
    try:
        try:
            n = int(text)
        except KeyError:
            out["inner"] = 1
            n = 0
    except ValueError as e:
        out["outer"] = type(e).__name__
        n = -1
    out["n"] = n
    return out


def order(xs, i):
    try:
        return xs[i]
    except LookupError:
        return "lookup"
    except IndexError:
        return "never"


if __name__ == "__main__":
    c1 = classify({"a": 1}, "a")                 #@value c1: exactly={"ok:1"}
    c2 = classify({}, "a")                       #@value c2: exactly={"key"}
    r1 = nested("12")
    #@row r1: present={start,n} definite={start,n} absent={inner,outer} closed
    n1 = r1["n"]                                 #@value n1: exactly={12}
    r2 = nested("x")
    #@row r2: present={start,outer,n} definite={start,outer,n} absent={inner} closed
    o2 = r2["outer"]                             #@value o2: exactly={"ValueError"}
    n2 = r2["n"]                                 #@value n2: exactly={-1}
    s1 = order([1, 2], 5)                        #@value s1: exactly={"lookup"}
    s2 = order([1, 2], 1)                        #@value s2: exactly={2}
    r3 = nested(sys.argv[-1])
    #@row r3: present={start,outer,n} definite={start,n} absent={inner} closed
