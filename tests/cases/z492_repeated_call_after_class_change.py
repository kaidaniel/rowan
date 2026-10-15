# A call made again at one site with the same arguments and the same names in its caller (a
# loop run to a fixpoint), after code between the two calls changed what the callee reads
# through a class attribute and through the receiver of a bound method it is given: the second
# call reads the new values (a call memo keyed by the entry would have to see these changes).
import os


class Config:
    mode = "a"


class Box:
    def __init__(self, v):
        self.v = v

    def get(self):
        return self.v


def get_mode():
    return Config.mode


def call(f):
    return f()


def main():
    n = len(os.environ.get("ROWAN_CASE_N", "ab"))
    m = None
    r = None
    b = Box(1)
    while n > 0:
        m = get_mode()
        r = call(b.get)
        Config.mode = "b"
        b = Box("x")
        n -= 1
    return m, r


m, r = main()  #@value m: exactly={None, 'a', 'b'}
#@value r: exactly={None, 1, 'x'}
#@work <= 600
