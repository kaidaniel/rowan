# A call made again at one site (a loop run to a fixpoint) after a key four levels below its
# argument was deleted: the callee's read fails on the second round.
import os

a = {"b": {"c": {"d": {"e": {"f": 1}}}}}


def f(x):
    return x["b"]["c"]["d"]["e"]["f"]  #@ possibly f


def main():
    n = len(os.environ.get("ROWAN_CASE_N", "ab"))
    r = 0
    while n > 0:
        r = f(a)
        del a["b"]["c"]["d"]["e"]["f"]  #@ ok f
        n -= 1
    return r


out = main()  #@value out: exactly={0, 1}
