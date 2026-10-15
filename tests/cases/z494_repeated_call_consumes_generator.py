# A call made again at one site with the same entry, where the callee consumes a generator
# made outside it: what the generator yields is the callee's result each time, and the second
# call finds the generator used up (a call memo would have to replay the consumption).
import os


def gen():
    yield "a"
    yield "b"


def first(it):
    for x in it:
        return x
    return "empty"


def main():
    n = len(os.environ.get("ROWAN_CASE_N", "ab"))
    g = gen()
    out = []
    while n > 0:
        out.append(first(g))
        n -= 1
    return out[0], out[-1]


a, z = main()  #@value a: includes={'a'}
#@todo value a: exactly={'a'}  (the first item appended in a loop run to a fixpoint: positions are joined)
#@value z: exactly={'a', 'b', 'empty'}
#@work <= 800
