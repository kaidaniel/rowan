# iter() of a generator returns the generator itself and runs none of its body: the body runs
# where the items are taken (the for loop, outside the try), so the KeyError of d["a"] is not
# the handler's (rowan ran the body at iter(), inside the try: r may have been "handler").
import json
import os


def gen(d):
    yield d["a"]


def f(d):
    try:
        it = iter(gen(d))
    except KeyError:
        return "handler"
    for x in it:
        pass
    return "loop"


d = json.loads(os.environ["Z640_D"]) if "Z640_D" in os.environ else {"a": 1}
r = f(d)
#@value r: exactly={'loop'}
