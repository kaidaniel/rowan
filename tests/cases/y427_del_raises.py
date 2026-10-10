"""Hypothesis: `del d[k]` of a key that is not there raises KeyError: a handler around it runs
(surely when the key is surely missing, maybe when it depends on input), the statements after
the del in the try body are skipped, and the dict keeps its keys."""
import json
import os


def remove(d, k):
    try:
        del d[k]
        return "removed"
    except KeyError as e:
        return "missing " + str(e)


a = {"x": 1}
r1 = remove(a, "y")                              #@value r1: exactly={"missing 'y'"}
_ = 0                                            #@row a: present={x} definite={x} closed
r2 = remove(a, "x")                              #@value r2: exactly={"removed"}
_ = 0                                            #@row a: present={} definite={} absent={x} closed
b = json.loads(os.environ.get("Y427_B", '{"k": 1}'))
r3 = remove(b, "k")                              #@value r3: includes={"removed","missing 'k'"}
