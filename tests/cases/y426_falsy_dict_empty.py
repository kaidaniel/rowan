"""Hypothesis: on the path where a dict tests falsy it is empty: len is 0 and no key is in it,
also for an open dict from the environment; a worklist drained by `while queue:` ends empty."""
import os


def drain(queue):
    seen = []
    while queue:
        k, _ = queue.popitem()
        seen.append(k)
    return len(queue), len(seen)


q = dict(os.environ)
n = -1
has = None
if not q:
    n = len(q)
    has = "PATH" in q
_ = 0                                            #@value n: exactly={0,-1}
_ = 0                                            #@value has: exactly={False,None}
q2 = {"a": 1, "b": 2}
left, done = drain(q2)                           #@value left: exactly={0}
_ = 0                                            #@row q2: present={} definite={} absent={a,b} closed
