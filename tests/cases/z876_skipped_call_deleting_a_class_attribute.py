"""A call that does not run (past its budget) of code that deletes a class attribute
(`del A.v`): after it the attribute may be gone — a read may raise AttributeError and a
second `del` may too. (Before, such a call only rebound it: the read gave its value or an
unknown, and the second `del` surely succeeded.)

Expectations were written from the program text before running the engine."""
#@limits budget=0


class A:
    v = 1


def keep():
    pass


def drop():
    del A.v


def run(f):
    f()


run(keep)
run(drop)
try:
    t = A.v
except AttributeError:
    t = "attr"
t_seen = t
#@value t_seen: includes={"attr"}
try:
    del A.v
    r = "deleted"
except AttributeError:
    r = "attr"
r_seen = r
#@value r_seen: includes={"attr"}
