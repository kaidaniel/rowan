"""Hypothesis: a while loop over a dict that the body both shrinks (popitem, LIFO) and grows
ends only when the dict is empty; the keys it adds are built from popped ones. Precise:
drain({"a": 3}) gives done {a: 3, spawn_a: 2, spawn_spawn_a: 1} and leaves the queue empty;
drain({"x": 1, "y": 1}) pops y first, so done's first key is "y"."""
import json
import os


def drain(queue):
    done = {}
    while queue:
        k, v = queue.popitem()
        done[k] = v
        if v > 1:
            queue["spawn_" + k] = v - 1
    return done


if __name__ == "__main__":
    q = {"a": 3}
    d1 = drain(q)
    #@row d1: present={a,spawn_a,spawn_spawn_a} definite={a,spawn_a,spawn_spawn_a} closed
    _ = 0                                        #@row q: present={} definite={} absent={a,spawn_a} closed
    v1 = d1["spawn_spawn_a"]                     #@value v1: exactly={1}
    n1 = len(d1)                                 #@value n1: exactly={3}
    q2 = {"x": 1, "y": 1}
    d2 = drain(q2)
    #@row d2: present={x,y} definite={x,y} absent={spawn_x,spawn_y} closed
    first = list(d2)[0]                          #@value first: exactly={"y"}
    q3 = json.loads(os.environ.get("Z433_Q", '{"m": 2}'))
    d3 = drain(q3)
    left = len(q3)                               #@value left: includes={0}
    e = drain({})
    #@row e: present={} definite={} closed
    #@value left: exactly={0}  (q3 is any JSON value: a falsy one that len() accepts is empty)
