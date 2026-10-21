# A decorator rowan does not model is taken to give the function back, but in fact gives back
# another object with attributes of its own (mock.patch's wrapper has `patchings`): reading
# one must not surely fail, nor hasattr be decided, as for a plain function it would.
import unittest.mock


@unittest.mock.patch("os.getcwd")
def check(m):
    return m


p = check.patchings
n = len(p)
#@value n: includes={1}
has = hasattr(check, "patchings")
#@value has: includes={True}
done = True
#@value done: exactly={True}


def plain():
    return 1


lacks = hasattr(plain, "patchings")
#@value lacks: exactly={False}
