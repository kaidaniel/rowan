# A line that runs several times (iterations of an unrolled loop, calls) may find a key on one
# run and miss it on another: it may fail, it does not fail every time.
import os

drop = os.environ.get("Z515_DROP", "") == "1"
last = os.environ.get("Z515_LAST", "1") == "1"

opts = {"k": 1}


def read_k():
    return opts["k"]  #@ possibly k


read_k()
if drop:
    del opts["k"]
read_k()

cfg = {"n": 1}
# (the second iteration's `del` fails: the program ends there)
for x in [0, 1]:
    if last:
        del cfg["n"]  #@ possibly n
