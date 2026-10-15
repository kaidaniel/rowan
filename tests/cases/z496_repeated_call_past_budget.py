# The same call made many times with the same entry while its function's budget runs out: the
# calls past the budget are unknown calls with what the function's code may change (it only
# stores "n": the key stays, the dict closed), and the value they leave is unknown (a call memo
# must not keep answering from a run made before the budget ran out).
#@limits budget=12
import os


def touch(d):
    d["n"] = d["n"] + 1
    return d["n"]


def main():
    d = {"n": 0}
    k = len(os.environ.get("ROWAN_CASE_N", "abcdefgh"))
    last = 0
    while k > 0:
        last = touch(d)
        k -= 1
    return d, last


d, last = main()
x = d["n"]  #@ ok n
#@row d: present={n} definite={n} closed
#@value x: unknown
