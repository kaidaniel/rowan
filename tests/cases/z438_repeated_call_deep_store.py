# A call made again at one site (a loop run to a fixpoint) after a store four levels below
# its argument: the second run reads the new value (a replay keyed by the argument's top
# levels only would give the first run's result again).
import os

a = {"b": {"c": {"d": {"e": {"f": 1}}}}}


def f(x):
    return x["b"]["c"]["d"]["e"]["f"]


def main():
    n = len(os.environ.get("ROWAN_CASE_N", "ab"))
    r = 0
    while n > 0:
        r = f(a)
        a["b"]["c"]["d"]["e"]["f"] = "two"
        n -= 1
    return r


out = main()  #@value out: exactly={0, 1, 'two'}
