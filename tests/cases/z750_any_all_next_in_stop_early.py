# any / all / next / `in` take a generator's items only until the answer is known, as CPython
# does: the items after the deciding one are never computed, so their reads, effects and raises
# do not happen (here the deciding item's truth is known).
f = {"a": 1}
r1 = any(f[k] > 0 for k in ["a", "b"])          #@ ok a ; none b
#@value r1: exactly={True}
e = {"a": 0}
r2 = all(e[k] > 0 for k in ["a", "b"])          #@ ok a ; none b
#@value r2: exactly={False}
r3 = 1 in (e[k] + 1 for k in ["a", "b"])        #@ ok a ; none b
#@value r3: exactly={True}
r4 = next(e[k] for k in ["a", "b"])             #@ ok a ; none b
#@value r4: exactly={0}
r5 = next((f[k] for k in ["b", "a"] if k in f), None)   #@ ok a ; none b
#@value r5: exactly={1}


def reads(m):
    yield m["a"] > 5                            #@ ok a
    yield m["b"] > 5                            #@ missing b


try:
    r6 = any(reads(f))
except KeyError:
    r6 = None
#@value r6: exactly={None}
r7 = 3 not in (e[k] for k in ["a", "b"] if k == "zz")
#@value r7: exactly={True}


def steps(out):
    out["one"] = 1
    yield 1
    out["two"] = 2
    yield 0
    out["three"] = 3


d1 = {}
s1 = any(steps(d1))
#@row d1: present={one} definite={one} closed
#@value s1: exactly={True}
d2 = {}
s2 = all(steps(d2))
#@row d2: present={one,two} definite={one,two} closed
#@value s2: exactly={False}
d3 = {}
s3 = 0 in steps(d3)
#@row d3: present={one,two} definite={one,two} closed
#@value s3: exactly={True}
d4 = {}
s4 = next(steps(d4))
#@row d4: present={one} definite={one} closed
#@value s4: exactly={1}
d5 = {}
s5 = any(x > 5 for x in steps(d5))
_ = 5
#@row d5: present={one,two,three} definite={one,two,three} closed
#@value s5: exactly={False}
d6 = {}
s6 = any(x > 0 for x in steps(d6))
_ = 6
#@row d6: present={one} definite={one} closed
#@value s6: exactly={True}
d7 = {}
s7 = next(steps(d7), None)
#@row d7: present={one} definite={one} closed
d8 = {}
s8 = all(x == 1 for x in steps(d8))
_ = 8
#@row d8: present={one,two} definite={one,two} closed
#@value s8: exactly={False}


def nothing(out):
    out["ran"] = 1
    return
    yield 1


d9 = {}
s9 = next(nothing(d9), "empty")
#@row d9: present={ran} definite={ran} closed
#@value s9: exactly={'empty'}
s10 = any(nothing(d9)) or all(nothing(d9))
#@value s10: exactly={True}
